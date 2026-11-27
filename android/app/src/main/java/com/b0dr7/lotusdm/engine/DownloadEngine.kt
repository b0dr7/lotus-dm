package com.b0dr7.lotusdm.engine

import android.content.Context
import android.net.ConnectivityManager
import android.net.NetworkCapabilities
import android.os.Handler
import android.os.Looper
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.coroutineScope
import kotlinx.coroutines.currentCoroutineContext
import kotlinx.coroutines.delay
import kotlinx.coroutines.isActive
import kotlinx.coroutines.launch
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

private fun partName(index: Int) = "part$index"

/**
 * Segmented HTTP engine: up to EIGHT parallel Range requests per file, merged
 * in order when they finish, falling back to a single stream when the server
 * ignores Range.
 *
 * No UI types live in here: it talks in DownloadJob and plain callbacks.
 */
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

    /** Global cap in bytes per second, 0 = unlimited. */
    @Volatile
    var speedLimit: Long = 0L

    /** When set, nothing runs unless the phone is on Wi-Fi. */
    @Volatile
    var wifiOnly: Boolean = false

    /** How many jobs may run at the same time; the rest wait by priority. */
    @Volatile
    var maxConcurrent: Int = MAX_CONCURRENT_DEFAULT

    @Volatile
    private var lastAllowed: Boolean = true

    init {
        restore()
    }

    private class Entry(
        val id: String,
        val url: String,
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

        /** True once the server proved it honours Range on this file. */
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

        /** Set while the Wi-Fi gate is holding this job back. */
        @Volatile
        var awaitingWifi: Boolean = false

        /** Backoff deadline of an automatic retry, 0 when there is none. */
        @Volatile
        var retryAt: Long = 0L

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
        )
    }

    fun observe(callback: ((List<DownloadJob>) -> Unit)?) {
        listener = callback
        callback?.let { publish(it, snapshot()) }
    }

    /** Priority first, then queue age - the same order the pump starts in. */
    private fun ordered(): List<Entry> = entries.values.sortedWith(
        compareByDescending<Entry> { it.priority.rank }.thenBy { it.created },
    )

    fun snapshot(): List<DownloadJob> = ordered().map { it.toPublic() }

    fun hasWork(): Boolean =
        entries.values.any { it.state == JobState.RUNNING || it.state == JobState.QUEUED }

    fun add(url: String): DownloadJob? {
        val clean = url.trim()
        if (clean.isEmpty()) return null
        if (!clean.startsWith("http://") && !clean.startsWith("https://")) return null

        val created = System.currentTimeMillis()
        val id = "$created-${(1000..9999).random()}"
        val dir = File(partsRoot(), id)
        dir.mkdirs()

        val entry = Entry(
            id = id,
            url = clean,
            fileName = guessFileName(clean, id),
            dir = dir,
            state = JobState.QUEUED,
            downloaded = 0L,
            total = 0L,
            error = null,
            limit = 0L,
            created = created,
        )
        entries[id] = entry
        persist(entry)
        publishStatus()
        pump()
        return entry.toPublic()
    }

    /** Explicit user start (or a retry). Respects the Wi-Fi gate. */
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
        entry.run?.cancel()
        entry.state = JobState.RUNNING
        entry.error = null
        entry.awaitingWifi = false
        entry.retryAt = 0L
        persist(entry)
        publishStatus()
        entry.run = scope.launch { download(entry) }
    }

    /**
     * Starts as many queued jobs as the concurrency setting allows, highest
     * priority first. Called after every state change that frees a slot.
     */
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

    /** Manual retry: clears the automatic counter and starts over. */
    fun retry(id: String) {
        val entry = entries[id] ?: return
        entry.attempts = 0
        start(id)
    }

    fun cancel(id: String) {
        val entry = entries[id] ?: return
        entry.state = JobState.PAUSED
        entry.run?.cancel()
        entry.run = null
        entries.remove(id)
        entry.dir.deleteRecursively()
        publishStatus()
        pump()
    }

    /** Drops finished rows but keeps the files they produced. */
    fun clearCompleted() {
        val finished = entries.values.filter { it.state == JobState.COMPLETED }
        finished.forEach { entry ->
            entries.remove(entry.id)
            entry.dir.deleteRecursively()
        }
        publishStatus()
        pump()
    }

    /** Removes a finished row and the file it created. */
    fun delete(id: String): Boolean {
        val entry = entries[id] ?: return false
        entry.run?.cancel()
        entry.run = null
        val removed = entry.uri?.let { uri ->
            CompletedFileStore.delete(appContext, uri, entry.savedPath)
        } ?: false
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

    // ------------------------------------------------------------- networks

    private fun isOnWifi(): Boolean {
        val manager = appContext.getSystemService(Context.CONNECTIVITY_SERVICE)
            as? ConnectivityManager ?: return true
        val network = manager.activeNetwork ?: return false
        val caps = manager.getNetworkCapabilities(network) ?: return false
        return caps.hasTransport(NetworkCapabilities.TRANSPORT_WIFI) ||
            caps.hasTransport(NetworkCapabilities.TRANSPORT_ETHERNET)
    }

    private fun networkAllowed(): Boolean = !wifiOnly || isOnWifi()

    /**
     * Called once a second by the foreground service. Holds the queue back on
     * mobile data and lets it go again when Wi-Fi comes back, without a
     * network callback the app would have to unregister.
     */
    /** Re-applies the Wi-Fi gate right after the setting changed. */
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

    // ---------------------------------------------------------------- work

    private suspend fun download(entry: Entry) {
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
                    // Server stopped honouring Range mid-flight: start clean.
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
            // A finished/paused job frees a slot for the next queued one.
            pump()
        }
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

    /** Ranged GET for one byte tells us both the size and whether Range works. */
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
