package com.b0dr7.lotusdm.engine

import android.content.Context
import android.media.MediaPlayer
import android.net.Uri
import android.net.ConnectivityManager
import android.net.NetworkCapabilities
import android.os.Handler
import android.os.Looper
import com.b0dr7.lotusdm.engine.torrent.TorrentClient
import com.b0dr7.lotusdm.engine.torrent.TorrentPublisher
import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.NonCancellable
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.coroutineScope
import kotlinx.coroutines.currentCoroutineContext
import kotlinx.coroutines.delay
import kotlinx.coroutines.isActive
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext
import okhttp3.OkHttpClient
import okhttp3.Request
import okhttp3.Response
import org.json.JSONArray
import org.json.JSONObject
import java.io.File
import java.io.IOException
import java.io.RandomAccessFile
import java.util.concurrent.ConcurrentHashMap
import java.util.concurrent.atomic.AtomicLongArray

private const val META_NAME = "meta.json"
private const val SINGLE_PART = "part"
private const val MERGED_NAME = "merged"
private const val MAX_SEGMENTS = 8
private const val MAX_CONCURRENT_DEFAULT = 3
private const val MIN_SEGMENTED_SIZE = 2L * 1024L * 1024L

private val RETRY_DELAYS = longArrayOf(30_000L, 120_000L, 600_000L)

private val VIDEO_QUALITIES = listOf("2160p", "1440p", "1080p", "720p", "480p", "360p", "240p")
private val AUDIO_FORMATS = listOf("mp3", "aac", "opus", "flac", "wav")
private const val DEFAULT_VIDEO_QUALITY = "720p"

fun isMediaUrl(url: String): Boolean {
    val lower = url.trim().lowercase()
    return lower.startsWith("https://www.youtube.com/") ||
        lower.startsWith("https://youtube.com/") ||
        lower.startsWith("https://m.youtube.com/") ||
        lower.startsWith("https://music.youtube.com/") ||
        lower.startsWith("https://youtube-nocookie.com/") ||
        lower.startsWith("https://youtu.be/") ||
        lower.startsWith("https://vimeo.com/")
}

fun isAudioQuality(quality: String?): Boolean = quality != null && quality in AUDIO_FORMATS

private fun partName(index: Int) = "part$index"

class DownloadEngine private constructor(context: Context) {

    private val appContext = context.applicationContext
    private val client = OkHttpClient.Builder()
        .followRedirects(true)
        .followSslRedirects(true)
        .build()
    private val scope = CoroutineScope(SupervisorJob() + Dispatchers.IO)
    private val main = Handler(Looper.getMainLooper())
    private val entries = ConcurrentHashMap<String, Entry>()

    @Volatile
    private var listener: ((List<DownloadJob>) -> Unit)? = null

    @Volatile
    var speedLimit: Long = 0L

    @Volatile
    var wifiOnly: Boolean = false

    @Volatile
    var maxConcurrent: Int = MAX_CONCURRENT_DEFAULT

    @Volatile
    private var lastAllowed: Boolean = true

    init {
        restore()
    }

    private class Entry(
        val id: String,
        @Volatile var url: String,
        @Volatile var fileName: String,
        val dir: File,
        @Volatile var state: JobState,
        @Volatile var downloaded: Long,
        @Volatile var total: Long,
        @Volatile var error: String?,
        @Volatile var limit: Long,
        val created: Long,
        @Volatile var tag: String = "",
        @Volatile var priority: JobPriority = JobPriority.NORMAL,
        @Volatile var kind: JobKind = JobKind.HTTP,
        @Volatile
        var awaitingWifi: Boolean = false,
        @Volatile
        var quality: String? = null,
        /** Mirror URLs tried in order if the primary URL fails. */
        @Volatile
        var mirrors: List<String> = emptyList(),
        @Volatile
        var startAt: Long = 0L,
    ) {
        @Volatile
        var speed: Long = 0L

        @Volatile
        var uri: String? = null

        @Volatile
        var savedPath: String? = null

        @Volatile
        var attempts: Int = 0

        @Volatile
        var segCount: Int = 0

        @Volatile
        var rangeOk: Boolean = false

        val segDone = AtomicLongArray(MAX_SEGMENTS)

        val lock = Any()

        @Volatile
        var windowBytes: Long = 0L

        @Volatile
        var windowStart: Long = 0L

        @Volatile
        var lastPublish: Long = 0L

        @Volatile
        var lastPersist: Long = 0L

        @Volatile
        var run: kotlinx.coroutines.Job? = null

        @Volatile
        var retryAt: Long = 0L

        @Volatile
        var torrentUri: String? = null

        @Volatile
        var torrentHash: String? = null

        fun toPublic(): DownloadJob = DownloadJob(
            id = id,
            url = url,
            fileName = fileName,
            state = state,
            downloaded = downloaded,
            total = total,
            speed = speed,
            error = error,
            uri = uri,
            mime = CompletedFileStore.mimeFor(fileName),
            limit = limit,
            tag = tag,
            priority = priority,
            kind = kind,
            awaitingWifi = awaitingWifi,
            quality = quality,
            mirrors = mirrors,
            startAt = startAt,
        )
    }

    fun observe(callback: ((List<DownloadJob>) -> Unit)?) {
        listener = callback
        callback?.let { publish(it, snapshot()) }
    }

    private fun ordered(): List<Entry> = entries.values.sortedWith(
        compareByDescending<Entry> { it.priority.rank }.thenBy { it.created },
    )

    fun snapshot(): List<DownloadJob> = ordered().map { it.toPublic() }

    fun hasWork(): Boolean =
        entries.values.any { it.state == JobState.RUNNING || it.state == JobState.QUEUED }

    private val torrents by lazy { TorrentClient(appContext) }

    private fun sanitizeName(raw: String?): String? {
        val cleaned = raw
            ?.trim()
            ?.replace(Regex("[\\\\/:*?\"<>|]"), "-")
            ?.trim('-', '.', ' ')
        return cleaned?.takeIf { it.isNotEmpty() }
    }

    fun add(url: String, name: String? = null, quality: String? = null, startAt: Long = 0L, mirrors: List<String> = emptyList()): DownloadJob? {
        val clean = url.trim()
        if (clean.isEmpty()) return null
        val wanted = sanitizeName(name)
        if (clean.startsWith("magnet:")) return enqueueTorrent(clean, wanted ?: magnetName(clean))
        if (!clean.startsWith("http://") && !clean.startsWith("https://")) return null
        val created = System.currentTimeMillis()
        val id = "$created-${(1000..9999).random()}"
        val dir = File(partsRoot(), id)
        dir.mkdirs()

        val entry = Entry(
            id = id,
            url = clean,
            fileName = wanted ?: guessFileName(clean, id),
            dir = dir,
            state = JobState.QUEUED,
            downloaded = 0L,
            total = 0L,
            error = null,
            limit = 0L,
            created = created,
            quality = if (isMediaUrl(clean) && quality != null) quality else null,
            mirrors = mirrors,
            startAt = if (startAt > 0L) startAt else 0L,
        )
        entries[id] = entry
        persist(entry)
        publishStatus()
        pump()
        return entry.toPublic()
    }

    fun addTorrentFile(file: File): DownloadJob? {
        if (!file.exists() || file.length() == 0L) return null
        return enqueueTorrent(file.absolutePath, file.name.removeSuffix(".torrent"))
    }

    private fun enqueueTorrent(uri: String, label: String): DownloadJob {
        val created = System.currentTimeMillis()
        val id = "t$created-${(1000..9999).random()}"
        val dir = File(partsRoot(), id)
        dir.mkdirs()

        var source = uri
        if (!uri.startsWith("magnet:")) {
            val original = File(uri)
            val copy = File(dir, "source.torrent")
            runCatching { original.copyTo(copy, overwrite = true) }
                .onSuccess { source = copy.absolutePath }
        }

        val entry = Entry(
            id = id,
            url = source,
            fileName = label.ifBlank { "Torrent" },
            dir = dir,
            state = JobState.QUEUED,
            downloaded = 0L,
            total = 0L,
            error = "Fetching torrent metadata",
            limit = 0L,
            created = created,
        )
        entry.kind = JobKind.TORRENT
        entry.torrentUri = source
        entries[id] = entry
        persist(entry)
        publishStatus()
        pump()
        return entry.toPublic()
    }

    private fun magnetName(magnet: String): String {
        val query = magnet.substringAfter('?', "")
        val display = query.split('&')
            .firstOrNull { it.startsWith("dn=") }
            ?.removePrefix("dn=")
            ?.let { java.net.URLDecoder.decode(it, "UTF-8") }
            ?.trim()
        return display.orEmpty().ifBlank { "Magnet download" }
    }

    fun start(id: String) {
        val entry = entries[id] ?: return
        if (entry.state == JobState.RUNNING) return
        entry.retryAt = 0L
        if (!networkAllowed()) {
            entry.awaitingWifi = true
            entry.state = JobState.QUEUED
            entry.error = "Waiting for Wi-Fi"
            persist(entry)
            publishStatus()
            return
        }
        begin(entry)
    }

    private fun begin(entry: Entry) {
        if (entry.startAt > 0L && entry.startAt > System.currentTimeMillis()) {
            entry.state = JobState.QUEUED
            entry.error = "Scheduled"
            persist(entry)
            publishStatus()
            return
        }
        entry.run?.cancel()
        entry.state = JobState.RUNNING
        entry.error = null
        entry.awaitingWifi = false
        entry.retryAt = 0L
        persist(entry)
        publishStatus()
        entry.run = scope.launch {
            if (entry.kind == JobKind.TORRENT) torrentDownload(entry) else if (entry.quality != null) mediaDownload(entry) else download(entry)
        }
    }

    fun pump() {
        if (!networkAllowed()) return
        var slots = maxConcurrent.coerceAtLeast(1) -
            entries.values.count { it.state == JobState.RUNNING }
        if (slots <= 0) return
        val now = System.currentTimeMillis()
        for (entry in ordered()) {
            if (slots <= 0) break
            if (entry.state != JobState.QUEUED || entry.run != null) continue
            if (entry.retryAt > now) continue
            if (entry.startAt > 0L && entry.startAt > now) continue
            begin(entry)
            slots--
        }
    }

    fun pause(id: String) {
        val entry = entries[id] ?: return
        if (entry.state != JobState.RUNNING) return
        entry.state = JobState.PAUSED
        entry.awaitingWifi = false
        entry.speed = 0L
        entry.torrentHash?.let { torrents.pause(it) }
        entry.run?.cancel()
        entry.run = null
        persist(entry)
        publishStatus()
        pump()
    }

    fun setTag(id: String, tag: String) {
        val entry = entries[id] ?: return
        entry.tag = tag
        persist(entry)
        publishStatus()
    }

    fun setPriority(id: String, priority: JobPriority) {
        val entry = entries[id] ?: return
        entry.priority = priority
        persist(entry)
        publishStatus()
        pump()
    }

    fun resume(id: String) = start(id)

    fun play(id: String) {
        val entry = entries[id] ?: return
        val uriString = entry.uri ?: return
        val uri = Uri.parse(uriString)
        val player = MediaPlayer.create(appContext, uri) ?: return
        player.start()
    }

    fun retry(id: String) {
        val entry = entries[id] ?: return
        entry.attempts = 0
        start(id)
    }

    fun cancel(id: String) {
        val entry = entries[id] ?: return
        entry.torrentHash?.let { torrents.remove(it) }
        entry.run?.cancel()
        entry.run = null
        entries.remove(id)
        entry.dir.deleteRecursively()
        publishStatus()
        pump()
    }

    fun clearCompleted() {
        val finished = entries.values.filter { it.state == JobState.COMPLETED }
        finished.forEach { entry ->
            entry.torrentHash?.let { torrents.remove(it) }
            entries.remove(entry.id)
            entry.dir.deleteRecursively()
        }
        publishStatus()
        pump()
    }

    fun delete(id: String): Boolean {
        val entry = entries[id] ?: return false
        entry.run?.cancel()
        entry.run = null
        entry.torrentHash?.let { torrents.remove(it) }
        val removed = if (entry.kind == JobKind.TORRENT) {
            true
        } else {
            entry.uri?.let { uri ->
                CompletedFileStore.delete(appContext, uri, entry.savedPath)
            } ?: false
        }
        entries.remove(id)
        entry.dir.deleteRecursively()
        publishStatus()
        pump()
        return removed
    }

    fun setLimit(id: String, bytesPerSecond: Long) {
        val entry = entries[id] ?: return
        entry.limit = bytesPerSecond
        persist(entry)
        publishStatus()
    }

    fun rename(id: String, newName: String) {
        val entry = entries[id] ?: return
        val clean = newName.trim()
        if (clean.isEmpty()) return
        entry.fileName = clean
        persist(entry)
        publishStatus()
    }


    private fun isOnWifi(): Boolean {
        val manager = appContext.getSystemService(Context.CONNECTIVITY_SERVICE)
            as? ConnectivityManager ?: return true
        val network = manager.activeNetwork ?: return false
        val caps = manager.getNetworkCapabilities(network) ?: return false
        return caps.hasTransport(NetworkCapabilities.TRANSPORT_WIFI) ||
            caps.hasTransport(NetworkCapabilities.TRANSPORT_ETHERNET)
    }

    private fun networkAllowed(): Boolean = !wifiOnly || isOnWifi()

    fun refreshNetwork() {
        lastAllowed = !networkAllowed()
        onNetworkTick()
    }

    fun onNetworkTick() {
        val allowed = networkAllowed()
        if (allowed == lastAllowed) {
            if (allowed) pump()
            return
        }
        lastAllowed = allowed
        if (allowed) releaseWifiHold() else holdForWifi()
    }

    private fun holdForWifi() {
        var touched = false
        for (entry in entries.values) {
            if (entry.state != JobState.RUNNING) continue
            entry.awaitingWifi = true
            entry.state = JobState.PAUSED
            entry.speed = 0L
            entry.error = "Paused: waiting for Wi-Fi"
            entry.torrentHash?.let { torrents.pause(it) }
            entry.run?.cancel()
            entry.run = null
            persist(entry)
            touched = true
        }
        if (touched) publishStatus()
    }

    private fun releaseWifiHold() {
        var touched = false
        for (entry in entries.values) {
            if (!entry.awaitingWifi) continue
            entry.awaitingWifi = false
            entry.error = null
            entry.state = JobState.QUEUED
            persist(entry)
            touched = true
        }
        if (touched) publishStatus()
        pump()
    }


    private suspend fun download(entry: Entry) {
        if (entry.mirrors.isNotEmpty()) {
            val urls = mutableListOf(entry.url)
            urls.addAll(entry.mirrors)
            for (url in urls) {
                val p = probe(url)
                if (p.total > 0L || p.rangeOk) {
                    entry.url = url
                    break
                }
            }
        }
        try {
            val probe = probe(entry.url)
            if (probe.total > 0L) entry.total = probe.total

            val segmented = probe.rangeOk &&
                probe.total >= MIN_SEGMENTED_SIZE &&
                (entry.segCount == 0 || entry.segCount == MAX_SEGMENTS)

            if (segmented) {
                entry.rangeOk = true
                entry.segCount = MAX_SEGMENTS
                try {
                    runSegmented(entry, probe.total)
                } catch (abort: RangeLostException) {
                    entry.segCount = 0
                    entry.segDone.set(0, 0L)
                    entry.downloaded = 0L
                    clearParts(entry)
                    runSingle(entry)
                }
            } else {
                entry.segCount = 0
                runSingle(entry)
            }
        } catch (t: Throwable) {
            if (entry.state == JobState.PAUSED) return
            handleFailure(entry, t)
        } finally {
            entry.run = null
            pump()
        }
    }

    private suspend fun torrentDownload(entry: Entry) {
        try {
            val source = entry.torrentUri ?: throw IOException("Missing torrent source")
            val client = torrents
            val limitKb = ((if (entry.limit > 0L) entry.limit else speedLimit) / 1024L).toInt()
            client.setDownloadLimit(limitKb)

            val hash = withContext(NonCancellable + Dispatchers.IO) {
                val known = entry.torrentHash
                val hash = if (known != null && client.resume(known)) {
                    known
                } else {
                    val added = client.add(source, entry.dir)
                    entry.torrentHash = added.hash
                    entry.fileName = added.name
                    if (added.total > 0L) entry.total = added.total
                    added.hash
                }
                if (entry.state != JobState.RUNNING) {
                    client.pause(hash)
                    return@withContext null
                }
                hash
            } ?: return
            entry.error = null
            persist(entry)
            publishStatus()

            while (currentCoroutineContext().isActive) {
                val progress = withContext(Dispatchers.IO) { client.status(hash) }
                    ?: throw IOException("Torrent dropped out of the session")
                entry.downloaded = progress.downloaded
                if (progress.total > 0L) entry.total = progress.total
                entry.speed = progress.speed
                val peersNote = if (progress.peers > 0) null else "Searching for peers"
                if (entry.error != peersNote) entry.error = peersNote
                publishStatus()

                if (progress.finished) {
                    withContext(Dispatchers.IO) { client.pause(hash) }
                    finishTorrent(entry)
                    return
                }
                delay(1000L)
            }
        } catch (cancel: CancellationException) {
            throw cancel
        } catch (t: Throwable) {
            if (entry.state == JobState.PAUSED) return
            handleFailure(entry, t)
        } finally {
            entry.run = null
            pump()
        }
    }

    private fun finishTorrent(entry: Entry) {
        val published = TorrentPublisher.publishDirectory(appContext, entry.dir, entry.fileName)
        entry.state = JobState.COMPLETED
        entry.speed = 0L
        if (entry.total <= 0L) entry.total = entry.downloaded
        entry.downloaded = entry.total
        entry.error = if (published) null else "Finished, but still in app storage"
        entry.uri = null
        persist(entry)
        publishStatus()
    }

    private fun handleFailure(entry: Entry, t: Throwable) {
        entry.speed = 0L
        entry.error = t.message ?: t::class.java.simpleName
        val attempt = entry.attempts

        if (attempt < RETRY_DELAYS.size) {
            val wait = RETRY_DELAYS[attempt]
            entry.attempts = attempt + 1
            entry.state = JobState.QUEUED
            entry.retryAt = System.currentTimeMillis() + wait
            entry.error = "Retrying in ${wait / 1000}s (attempt ${attempt + 1} of ${RETRY_DELAYS.size})"
            persist(entry)
            publishStatus()
            scope.launch {
                delay(wait)
                val current = entries[entry.id] ?: return@launch
                if (current.state == JobState.QUEUED) current.retryAt = 0L
                pump()
            }
            return
        }

        entry.state = JobState.FAILED
        entry.retryAt = 0L
        persist(entry)
        publishStatus()
        pump()
    }

    private fun probe(url: String): Probe {
        val request = Request.Builder().url(url).header("Range", "bytes=0-0").build()
        val response = runCatching { client.newCall(request).execute() }.getOrNull()
            ?: return Probe(0L, false)

        return response.use { r ->
            val ranged = r.header("Content-Range")?.substringAfterLast('/')?.toLongOrNull() ?: 0L
            if (r.code == 206 && ranged > 0L) {
                Probe(ranged, true)
            } else {
                Probe(r.header("Content-Length")?.toLongOrNull() ?: 0L, false)
            }
        }
    }

    private suspend fun runSegmented(entry: Entry, total: Long) {
        val count = MAX_SEGMENTS
        val base = total / count
        val remainder = total % count

        val starts = LongArray(count)
        val ends = LongArray(count)
        var cursor = 0L
        for (index in 0 until count) {
            val size = base + if (index < remainder) 1L else 0L
            starts[index] = cursor
            ends[index] = cursor + size - 1L
            cursor += size
        }

        coroutineScope {
            for (index in 0 until count) {
                launch { runSegment(entry, index, starts[index], ends[index], count) }
            }
        }

        if (entry.downloaded < total) {
            throw IOException("Only ${entry.downloaded} of $total bytes arrived")
        }

        val merged = merge(entry, count)
        val published = CompletedFileStore.publish(appContext, merged, entry.fileName)
        if (published != null) {
            entry.uri = published.uri
            entry.savedPath = published.path
            entry.fileName = published.displayName
            entry.error = null
        } else {
            entry.error = "Saved, but not in Downloads"
        }
        entry.state = JobState.COMPLETED
        entry.speed = 0L
        entry.total = total
        entry.downloaded = total
        persist(entry)
        publishStatus()
        clearParts(entry)
    }

    private suspend fun runSegment(
        entry: Entry,
        index: Int,
        start: Long,
        end: Long,
        count: Int,
    ) {
        val length = end - start + 1L
        val partFile = File(entry.dir, partName(index))

        if (entry.segDone.get(index) > 0L && !partFile.exists()) {
            entry.segDone.set(index, 0L)
        }
        var done = entry.segDone.get(index)
        if (done >= length) return

        val from = start + done
        val request = Request.Builder()
            .url(entry.url)
            .header("Range", "bytes=$from-$end")
            .build()

        val response = client.newCall(request).execute()
        response.use { resp ->
            if (resp.code == 200) throw RangeLostException()
            if (resp.code != 206) throw IOException("HTTP ${resp.code} on segment ${index + 1}")

            val body = resp.body ?: throw IOException("Empty segment body")
            body.byteStream().use { input ->
                RandomAccessFile(partFile, "rw").use { raf ->
                    raf.seek(done)
                    val buffer = ByteArray(32 * 1024)
                    var windowStart = System.currentTimeMillis()
                    var windowBytes = 0L

                    while (currentCoroutineContext().isActive) {
                        val read = input.read(buffer)
                        if (read <= 0) break

                        raf.write(buffer, 0, read)
                        done += read
                        entry.segDone.set(index, done)
                        onBytes(entry, read.toLong())

                        val now = System.currentTimeMillis()
                        val elapsed = now - windowStart
                        windowBytes += read
                        if (elapsed >= 250L) {
                            val limit = if (entry.limit > 0L) entry.limit else speedLimit
                            if (limit > 0L) {
                                val perSegment = (limit / count).coerceAtLeast(1L)
                                val expected = windowBytes * 1000L / perSegment
                                if (expected > elapsed) delay(expected - elapsed)
                            }
                            windowBytes = 0L
                            windowStart = System.currentTimeMillis()
                        }
                    }
                }
            }
        }

        if (done < length) {
            throw IOException("Segment ${index + 1} stopped at $done of $length bytes")
        }
    }

    private suspend fun runSingle(entry: Entry) {
        val part = File(entry.dir, SINGLE_PART)
        val existing = if (part.exists()) part.length() else 0L

        val builder = Request.Builder().url(entry.url)
        if (existing > 0L) builder.header("Range", "bytes=$existing-")

        val response = client.newCall(builder.build()).execute()
        response.use { resp ->
            if (!resp.isSuccessful) throw IOException("HTTP ${resp.code}")

            val body = resp.body ?: throw IOException("Empty response body")
            val resuming = resp.code == 206 && existing > 0L
            if (!resuming) part.delete()

            val startAt = if (resuming) existing else 0L
            val remaining = body.contentLength()
            entry.total = when {
                remaining <= 0L -> 0L
                resuming -> startAt + remaining
                else -> remaining
            }
            entry.downloaded = startAt
            entry.speed = 0L
            publishStatus()

            body.byteStream().use { input ->
                RandomAccessFile(part, "rw").use { raf ->
                    raf.seek(startAt)
                    val buffer = ByteArray(64 * 1024)
                    var windowStart = System.currentTimeMillis()
                    var windowBytes = 0L

                    while (currentCoroutineContext().isActive) {
                        val read = input.read(buffer)
                        if (read <= 0) break

                        raf.write(buffer, 0, read)
                        entry.downloaded += read
                        windowBytes += read

                        val now = System.currentTimeMillis()
                        val elapsed = now - windowStart
                        if (elapsed >= 300L) {
                            entry.speed = windowBytes * 1000L / elapsed
                            windowBytes = 0L
                            windowStart = now
                        }
                        if (now - entry.lastPublish >= 500L) {
                            entry.lastPublish = now
                            publishStatus()
                        }

                        val limit = if (entry.limit > 0L) entry.limit else speedLimit
                        if (limit > 0L && elapsed >= 300L) {
                            val expected = windowBytes * 1000L / limit
                            if (expected > elapsed) delay(expected - elapsed)
                        }
                    }
                }
            }
        }

        if (entry.downloaded <= 0L || (entry.total > 0L && entry.downloaded < entry.total)) {
            throw IOException("Connection closed before the file finished")
        }

        val published = CompletedFileStore.publish(appContext, part, entry.fileName)
        if (published != null) {
            entry.uri = published.uri
            entry.savedPath = published.path
            entry.fileName = published.displayName
            entry.error = null
        } else {
            entry.error = "Saved, but not in Downloads"
        }
        entry.state = JobState.COMPLETED
        entry.speed = 0L
        if (entry.total <= 0L) entry.total = entry.downloaded
        persist(entry)
        publishStatus()
    }

    private suspend fun mediaDownload(entry: Entry) {
        try {
            val quality = entry.quality ?: throw IOException("Missing media quality")
            val isVideo = quality.endsWith('p') && quality.dropLast(1).toIntOrNull() != null
            val height = quality.removeSuffix("p").toIntOrNull() ?: 720
            val stem = entry.url.substringBefore('?').substringBefore('#').substringAfterLast('/')
                .replace(Regex("[^A-Za-z0-9._ -]"), "_").trim()
            val base = if (isVideo) "${stem}-$height" else stem
            val formatArg = if (isVideo) "bestvideo[height<={$height}]+bestaudio/best[height<={$height}]" else "bestaudio/best"
            val args = mutableListOf(
                "--no-playlist",
                "-f", formatArg,
            )
            if (!isVideo) {
                args.add("--extract-audio")
                args.add("--audio-format")
                args.add(if (isVideo) "mp4" else quality)
            }
            args.add("-o")
            args.add(File(entry.dir, "${base}.%(ext)s").absolutePath)
            args.add(entry.url)

            val process = ProcessBuilder(
                File(appContext.filesDir, "yt-dlp").absolutePath,
                *args.toTypedArray(),
            ).start()
            process.inputStream.bufferedReader().readText()
            val finished = process.waitFor()
            if (finished != 0) {
                throw IOException("yt-dlp exited with $finished")
            }
            val finalFile = entry.dir.listFiles()
                ?.firstOrNull { it.isFile && it.name.startsWith("$base.") }
                ?: throw IOException("yt-dlp did not produce a file")
            val published = CompletedFileStore.publish(appContext, finalFile, base)
            if (published == null) throw IOException("Saved, but not in Downloads")
            entry.uri = published.uri
            entry.savedPath = published.path
            entry.fileName = published.displayName
            entry.state = JobState.COMPLETED
            entry.speed = 0L
            entry.downloaded = finalFile.length()
            entry.total = finalFile.length()
            entry.error = null
            persist(entry)
            publishStatus()
        } catch (t: Throwable) {
            if (entry.state == JobState.PAUSED) return
            handleFailure(entry, t)
        } finally {
            entry.run = null
            pump()
        }
    }

    private fun onBytes(entry: Entry, count: Long) {
        val now = System.currentTimeMillis()
        synchronized(entry.lock) {
            entry.downloaded = if (entry.segCount > 0) {
                var sum = 0L
                for (index in 0 until entry.segCount) sum += entry.segDone.get(index)
                sum
            } else {
                entry.downloaded + count
            }
            entry.windowBytes += count
            val elapsed = now - entry.windowStart
            if (elapsed >= 500L) {
                entry.speed = entry.windowBytes * 1000L / elapsed
                entry.windowBytes = 0L
                entry.windowStart = now
            }
        }

        if (now - entry.lastPublish >= 500L) {
            entry.lastPublish = now
            publishStatus()
        }
        if (now - entry.lastPersist >= 2000L) {
            entry.lastPersist = now
            persist(entry)
        }
    }

    private fun merge(entry: Entry, count: Int): File {
        val merged = File(entry.dir, MERGED_NAME)
        if (merged.exists()) merged.delete()

        merged.outputStream().use { output ->
            for (index in 0 until count) {
                val part = File(entry.dir, partName(index))
                if (!part.exists()) throw IOException("Part ${index + 1} is missing")
                part.inputStream().use { it.copyTo(output, 64 * 1024) }
            }
        }
        return merged
    }

    private fun clearParts(entry: Entry) {
        for (index in 0 until MAX_SEGMENTS) {
            File(entry.dir, partName(index)).delete()
        }
        File(entry.dir, SINGLE_PART).delete()
        File(entry.dir, MERGED_NAME).delete()
    }

    private fun persist(entry: Entry) {
        runCatching {
            synchronized(entry.lock) {
                entry.dir.mkdirs()
                val segments = JSONArray()
                var summed = entry.downloaded
                if (entry.segCount > 0) {
                    summed = 0L
                    for (index in 0 until entry.segCount) {
                        val value = entry.segDone.get(index)
                        segments.put(value)
                        summed += value
                    }
                }

                val json = JSONObject()
                    .put("id", entry.id)
                    .put("url", entry.url)
                    .put("fileName", entry.fileName)
                    .put("state", entry.state.name)
                    .put("downloaded", summed)
                    .put("total", entry.total)
                    .put("created", entry.created)
                    .put("uri", entry.uri ?: "")
                    .put("savedPath", entry.savedPath ?: "")
                    .put("segCount", entry.segCount)
                    .put("segments", segments)
                    .put("attempts", entry.attempts)
                    .put("limit", entry.limit)
                    .put("tag", entry.tag)
                    .put("priority", entry.priority.name)
                    .put("kind", entry.kind.name)
                    .put("torrentUri", entry.torrentUri ?: "")
                    .put("quality", entry.quality ?: "")
                    .put("startAt", entry.startAt)
                    .put("mirrors", org.json.JSONArray(entry.mirrors))
                File(entry.dir, META_NAME).writeText(json.toString())
            }
        }
    }

    private fun restore() {
        val root = partsRoot()
        if (!root.exists()) return

        root.listFiles()?.forEach { dir ->
            if (!dir.isDirectory) return@forEach
            val meta = File(dir, META_NAME)
            if (!meta.exists()) return@forEach

            runCatching {
                val json = JSONObject(meta.readText())
                val id = json.optString("id", dir.name)
                val state = when (json.optString("state", JobState.PAUSED.name)) {
                    JobState.COMPLETED.name -> JobState.COMPLETED
                    JobState.FAILED.name -> JobState.FAILED
                    else -> JobState.PAUSED
                }

                val entry = Entry(
                    id = id,
                    url = json.getString("url"),
                    fileName = json.optString("fileName", id),
                    dir = dir,
                    state = state,
                    downloaded = json.optLong("downloaded", 0L),
                    total = json.optLong("total", 0L),
                    error = null,
                    limit = json.optLong("limit", 0L),
                    created = json.optLong("created", dir.lastModified()),
                )
                entry.uri = json.optString("uri", "").ifEmpty { null }
                entry.savedPath = json.optString("savedPath", "").ifEmpty { null }
                entry.attempts = json.optInt("attempts", 0)
                entry.segCount = json.optInt("segCount", 0)
                entry.tag = json.optString("tag", "")
                entry.priority = JobPriority.entries
                    .firstOrNull { it.name == json.optString("priority", "") }
                    ?: JobPriority.NORMAL
                entry.kind = JobKind.entries
                    .firstOrNull { it.name == json.optString("kind", "") }
                    ?: JobKind.HTTP
                entry.torrentUri = json.optString("torrentUri", "").ifEmpty { null }
                entry.quality = json.optString("quality", "").ifEmpty { null }
                entry.startAt = json.optLong("startAt", 0L)
                val mirrorsArr = json.optJSONArray("mirrors")
                if (mirrorsArr != null) {
                    val list = mutableListOf<String>()
                    for (i in 0 until mirrorsArr.length()) list.add(mirrorsArr.optString(i))
                    entry.mirrors = list
                }

                val segments = json.optJSONArray("segments")
                if (segments != null && entry.segCount > 0) {
                    for (index in 0 until minOf(entry.segCount, segments.length())) {
                        entry.segDone.set(index, segments.optLong(index, 0L))
                    }
                }
                entries[id] = entry
            }
        }
    }

    private fun publishStatus() {
        val callback = listener ?: return
        publish(callback, snapshot())
    }

    private fun publish(callback: (List<DownloadJob>) -> Unit, jobs: List<DownloadJob>) {
        main.post { callback(jobs) }
    }

    private fun partsRoot(): File {
        val base = appContext.getExternalFilesDir(null) ?: appContext.filesDir
        return File(base, "parts")
    }

    private fun guessFileName(url: String, fallback: String): String {
        val path = url.substringBefore('?').substringBefore('#')
        val raw = path.substringAfterLast('/')
        val cleaned = raw.replace(Regex("[^A-Za-z0-9._ -]"), "_").trim()
        return if (cleaned.isEmpty() || cleaned == "." || cleaned == "..") fallback else cleaned
    }

    private class Probe(val total: Long, val rangeOk: Boolean)

    private class RangeLostException : IOException("Server stopped honouring Range")

    companion object {
        @Volatile
        private var instance: DownloadEngine? = null

        fun get(context: Context): DownloadEngine {
            instance?.let { return it }
            return synchronized(this) {
                instance ?: DownloadEngine(context).also { instance = it }
            }
        }
    }
}
