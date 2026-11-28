package com.b0dr7.lotusdm.engine.torrent

import android.content.Context
import com.frostwire.jlibtorrent.SessionManager
import com.frostwire.jlibtorrent.TorrentHandle
import com.frostwire.jlibtorrent.TorrentInfo
import java.io.File
import java.io.IOException

/** One snapshot of a running torrent, in the units the queue already uses. */
data class TorrentProgress(
    val downloaded: Long,
    val total: Long,
    val speed: Long,
    val peers: Int,
    val finished: Boolean,
)

/**
 * The BitTorrent side of the queue. One libtorrent session per process, owned
 * by the foreground service, so transfers keep going while the UI is closed.
 */
class TorrentClient(context: Context) {

    private val app = context.applicationContext
    private val manager = SessionManager()
    private val metaDir = File(app.cacheDir, "torrent-meta").apply { mkdirs() }

    private val lock = Any()
    private val infos = HashMap<String, TorrentInfo>()

    /** Hashes whose handle has actually been seen, see [status]. */
    private val live = HashSet<String>()
    private var started = false

    data class Added(val hash: String, val name: String, val total: Long)

    private fun ensureStarted() = synchronized(lock) {
        if (started) return@synchronized
        metaDir.mkdirs()
        manager.start()
        started = true
    }

    /**
     * Adds a magnet link or a .torrent file. Magnet metadata is fetched up
     * front (blocking, so call it off the main thread) because the queue needs
     * a real name and size before it can show progress.
     */
    fun add(uri: String, saveDir: File): Added {
        ensureStarted()
        saveDir.mkdirs()

        val info = loadInfo(uri)
        val name = runCatching { info.name() }.getOrNull()?.takeIf { it.isNotBlank() } ?: "Torrent"
        val total = runCatching { info.totalSize() }.getOrDefault(0L)
        val hash = runCatching {
            info.infoHashV1()?.toHex() ?: "$name-$total"
        }.getOrNull() ?: "$name-$total"

        val existing = handle(info)
        if (existing == null) {
            runCatching { manager.download(info, saveDir) }.onFailure {
                throw IOException(it.message ?: "libtorrent refused this torrent")
            }
        } else {
            // Already in the session (a paused row that was restarted): the
            // old handle is still paused, so resume it rather than adding a
            // second copy libtorrent would refuse.
            runCatching { existing.resume() }
        }

        synchronized(lock) { infos[hash] = info }
        return Added(hash, name, total)
    }

    /**
     * Null once the torrent is no longer in the session. A handle that was
     * alive and then vanished is gone for good, so the caller retries; a
     * handle that was never seen yet (the poll right after adding) still gets
     * a zero snapshot instead of a spurious failure.
     */
    fun status(hash: String): TorrentProgress? {
        val info = synchronized(lock) { infos[hash] } ?: return null
        val declared = runCatching { info.totalSize() }.getOrDefault(0L)
        val status = handle(info)?.let { runCatching { it.status() }.getOrNull() }
        if (status == null) {
            val wasLive = synchronized(lock) { live.remove(hash) }
            return if (wasLive) null else TorrentProgress(0L, declared, 0L, 0, false)
        }
        synchronized(lock) { live.add(hash) }

        return TorrentProgress(
            downloaded = status.totalDone(),
            total = if (declared > 0L) declared else status.totalDone(),
            speed = status.downloadRate().toLong().coerceAtLeast(0L),
            peers = status.numPeers(),
            finished = status.isFinished() || status.isSeeding(),
        )
    }

    fun pause(hash: String) {
        val info = synchronized(lock) { infos[hash] } ?: return
        handle(info)?.let { runCatching { it.pause() } }
    }

    /**
     * Resumes a torrent that is still in the session. Returns false when the
     * session no longer knows it (a restart or a removal), so the caller can
     * re-add it from its source instead.
     */
    fun resume(hash: String): Boolean {
        val info = synchronized(lock) { infos[hash] } ?: return false
        val handle = handle(info) ?: return false
        return runCatching { handle.resume() }.isSuccess
    }

    /** Drops the torrent from the session; files are left for the caller. */
    fun remove(hash: String) {
        val info = synchronized(lock) {
            live.remove(hash)
            infos.remove(hash)
        } ?: return
        val handle = handle(info) ?: return
        runCatching { manager.remove(handle) }
    }

    /** Session-wide download cap in KB/s, 0 means unlimited. */
    fun setDownloadLimit(kb: Int) {
        if (!started) return
        runCatching { manager.downloadRateLimit(kb.coerceAtLeast(0)) }
    }

    private fun handle(info: TorrentInfo): TorrentHandle? =
        runCatching { manager.find(info) }.getOrNull()

    private fun loadInfo(uri: String): TorrentInfo {
        if (uri.startsWith("magnet:")) {
            val data = manager.fetchMagnet(uri, MAGNET_TIMEOUT_SECONDS, metaDir)
                ?: throw IOException("No metadata came back for this magnet link")
            return TorrentInfo(data)
        }
        val path = uri.removePrefix("file://")
        val file = File(path)
        if (!file.exists() || file.length() == 0L) throw IOException("Torrent file is gone")
        return TorrentInfo(file)
    }

    private companion object {
        const val MAGNET_TIMEOUT_SECONDS = 45
    }
}
