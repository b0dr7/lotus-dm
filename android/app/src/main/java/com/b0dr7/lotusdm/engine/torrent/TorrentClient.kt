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
    private var started = false

    data class Added(val hash: String, val name: String, val total: Long)

    private fun ensureStarted() = synchronized(lock) {
        if (started) return@synchronized
        metaDir.mkdirs()
        manager.start()
        started = true
    }

    fun shutdown() {
        synchronized(lock) {
            if (!started) return
            started = false
            infos.clear()
        }
        runCatching { manager.stop() }
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

        if (runCatching { manager.find(info) }.getOrNull() == null) {
            runCatching { manager.download(info, saveDir) }.onFailure {
                throw IOException(it.message ?: "libtorrent refused this torrent")
            }
        }

        synchronized(lock) { infos[hash] = info }
        return Added(hash, name, total)
    }

    /** Null once the torrent is no longer in the session. */
    fun status(hash: String): TorrentProgress? {
        val info = synchronized(lock) { infos[hash] } ?: return null
        val handle = runCatching { manager.find(info) }.getOrNull()
        val status = handle?.let { runCatching { it.status() }.getOrNull() }
            ?: return TorrentProgress(0L, runCatching { info.totalSize() }.getOrDefault(0L), 0L, 0, false)

        val declared = runCatching { info.totalSize() }.getOrDefault(0L)
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

    fun resume(hash: String) {
        val info = synchronized(lock) { infos[hash] } ?: return
        handle(info)?.let { runCatching { it.resume() } }
    }

    /** Drops the torrent from the session; files are left for the caller. */
    fun remove(hash: String) {
        val info = synchronized(lock) { infos.remove(hash) } ?: return
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
