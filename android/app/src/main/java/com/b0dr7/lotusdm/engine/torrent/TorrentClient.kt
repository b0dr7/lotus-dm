package com.b0dr7.lotusdm.engine.torrent

import android.content.Context
import com.frostwire.jlibtorrent.SessionManager
import com.frostwire.jlibtorrent.TorrentHandle
import com.frostwire.jlibtorrent.TorrentInfo
import java.io.File
import java.io.IOException

data class TorrentProgress(
    val downloaded: Long,
    val total: Long,
    val speed: Long,
    val peers: Int,
    val finished: Boolean,
)

class TorrentClient(context: Context) {

    private val app = context.applicationContext
    private val manager = SessionManager()
    private val metaDir = File(app.cacheDir, "torrent-meta").apply { mkdirs() }

    private val lock = Any()
    private val infos = HashMap<String, TorrentInfo>()

    private val live = HashSet<String>()
    private var started = false

    data class Added(val hash: String, val name: String, val total: Long)

    private fun ensureStarted() = synchronized(lock) {
        if (started) return@synchronized
        metaDir.mkdirs()
        manager.start()
        started = true
    }

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
            runCatching { existing.resume() }
        }

        synchronized(lock) { infos[hash] = info }
        return Added(hash, name, total)
    }

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

    fun resume(hash: String): Boolean {
        val info = synchronized(lock) { infos[hash] } ?: return false
        val handle = handle(info) ?: return false
        return runCatching { handle.resume() }.isSuccess
    }

    fun remove(hash: String) {
        val info = synchronized(lock) {
            live.remove(hash)
            infos.remove(hash)
        } ?: return
        val handle = handle(info) ?: return
        runCatching { manager.remove(handle) }
    }

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
        val path = uri.removePrefix("file:
        val file = File(path)
        if (!file.exists() || file.length() == 0L) throw IOException("Torrent file is gone")
        return TorrentInfo(file)
    }

    private companion object {
        const val MAGNET_TIMEOUT_SECONDS = 45
    }
}
