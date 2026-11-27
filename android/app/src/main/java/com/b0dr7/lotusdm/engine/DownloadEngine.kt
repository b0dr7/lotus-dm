package com.b0dr7.lotusdm.engine

import android.content.Context
import android.os.Handler
import android.os.Looper
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.currentCoroutineContext
import kotlinx.coroutines.delay
import kotlinx.coroutines.isActive
import kotlinx.coroutines.launch
import okhttp3.OkHttpClient
import okhttp3.Request
import okhttp3.Response
import org.json.JSONObject
import java.io.File
import java.io.IOException
import java.io.RandomAccessFile
import java.util.concurrent.ConcurrentHashMap

/**
 * Single-stream engine with HTTP Range resume.
 *
 * No UI types live in here: it talks in DownloadJob and plain callbacks, so
 * the desktop and Android engines can stay conceptually identical.
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
    ) {
        @Volatile
        var speed: Long = 0L

        @Volatile
        var uri: String? = null

        @Volatile
        var savedPath: String? = null

        @Volatile
        var run: kotlinx.coroutines.Job? = null

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
        )
    }

    fun observe(callback: ((List<DownloadJob>) -> Unit)?) {
        listener = callback
        callback?.let { publish(it, snapshot()) }
    }

    fun snapshot(): List<DownloadJob> =
        entries.values.sortedBy { it.created }.map { it.toPublic() }

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
        start(id)
        return entry.toPublic()
    }

    fun start(id: String) {
        val entry = entries[id] ?: return
        if (entry.state == JobState.RUNNING) return
        entry.run?.cancel()
        entry.state = JobState.RUNNING
        entry.error = null
        persist(entry)
        publishStatus()
        entry.run = scope.launch { download(entry) }
    }

    fun pause(id: String) {
        val entry = entries[id] ?: return
        if (entry.state != JobState.RUNNING) return
        entry.state = JobState.PAUSED
        entry.speed = 0L
        entry.run?.cancel()
        entry.run = null
        persist(entry)
        publishStatus()
    }

    fun resume(id: String) = start(id)

    fun cancel(id: String) {
        val entry = entries[id] ?: return
        entry.state = JobState.PAUSED
        entry.run?.cancel()
        entry.run = null
        entries.remove(id)
        entry.dir.deleteRecursively()
        publishStatus()
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
        return removed
    }

    fun setLimit(id: String, bytesPerSecond: Long) {
        entries[id]?.limit = bytesPerSecond
    }

    private suspend fun download(entry: Entry) {
        var response: Response? = null
        try {
            val part = File(entry.dir, PART_NAME)
            val existing = if (part.exists()) part.length() else 0L

            val builder = Request.Builder().url(entry.url)
            if (existing > 0L) builder.header("Range", "bytes=$existing-")

            response = client.newCall(builder.build()).execute()
            if (!response.isSuccessful) throw IOException("HTTP ${response.code}")

            val body = response.body ?: throw IOException("Empty response body")
            val resuming = response.code == 206 && existing > 0L
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
                    var lastPublish = 0L

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
                        if (now - lastPublish >= 500L) {
                            lastPublish = now
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

            if (entry.downloaded > 0L && (entry.total <= 0L || entry.downloaded >= entry.total)) {
                complete(entry)
            } else if (currentCoroutineContext().isActive) {
                throw IOException("Connection closed before the file finished")
            }
        } catch (t: Throwable) {
            if (entry.state == JobState.PAUSED) return
            entry.state = JobState.FAILED
            entry.error = t.message ?: t::class.java.simpleName
            entry.speed = 0L
            persist(entry)
            publishStatus()
        } finally {
            response?.close()
            entry.run = null
        }
    }

    private fun complete(entry: Entry) {
        val part = File(entry.dir, PART_NAME)
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

    private fun persist(entry: Entry) {
        runCatching {
            entry.dir.mkdirs()
            val json = JSONObject()
                .put("id", entry.id)
                .put("url", entry.url)
                .put("fileName", entry.fileName)
                .put("state", entry.state.name)
                .put("downloaded", entry.downloaded)
                .put("total", entry.total)
                .put("created", entry.created)
                .put("uri", entry.uri ?: "")
                .put("savedPath", entry.savedPath ?: "")
            File(entry.dir, META_NAME).writeText(json.toString())
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
                    limit = 0L,
                    created = json.optLong("created", dir.lastModified()),
                )
                entry.uri = json.optString("uri", "").ifEmpty { null }
                entry.savedPath = json.optString("savedPath", "").ifEmpty { null }
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

    companion object {
        private const val META_NAME = "meta.json"
        private const val PART_NAME = "part"

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
