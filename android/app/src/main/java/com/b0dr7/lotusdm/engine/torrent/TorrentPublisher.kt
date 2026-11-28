package com.b0dr7.lotusdm.engine.torrent

import android.content.ContentValues
import android.content.Context
import android.os.Build
import android.os.Environment
import android.provider.MediaStore
import com.b0dr7.lotusdm.engine.CompletedFileStore
import java.io.File

object TorrentPublisher {

    private const val FOLDER = "LotusDM"
    private const val RELATIVE = "Download/$FOLDER"

    fun publishDirectory(context: Context, source: File, name: String): Boolean {
        if (!source.isDirectory) return false
        val files = source.walkTopDown().filter { it.isFile }.toList()
        if (files.isEmpty()) return false

        val safe = safeName(name)
        val moved = if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.Q) {
            copyThroughMediaStore(context, source, files, safe)
        } else {
            moveLegacy(context, source, safe)
        }
        if (moved) source.deleteRecursively()
        return moved
    }


    private fun copyThroughMediaStore(
        context: Context,
        root: File,
        files: List<File>,
        safe: String,
    ): Boolean {
        val resolver = context.contentResolver
        var complete = true
        for (file in files) {
            val relative = file.relativeTo(root).path.replace('\\', '/')
            val parent = relative.substringBeforeLast('/', "")
            val relativePath = buildString {
                append(RELATIVE).append('/').append(safe).append('/')
                if (parent.isNotEmpty()) append(parent).append('/')
            }
            val values = ContentValues().apply {
                put(MediaStore.MediaColumns.DISPLAY_NAME, file.name)
                put(MediaStore.MediaColumns.MIME_TYPE, CompletedFileStore.mimeFor(file.name))
                put(MediaStore.MediaColumns.RELATIVE_PATH, relativePath)
                put(MediaStore.MediaColumns.IS_PENDING, 1)
            }
            val uri = runCatching {
                resolver.insert(MediaStore.Downloads.EXTERNAL_CONTENT_URI, values)
            }.getOrNull()
            if (uri == null) {
                complete = false
                continue
            }
            val copied = runCatching {
                resolver.openOutputStream(uri)?.use { output ->
                    file.inputStream().use { input -> input.copyTo(output, 64 * 1024) }
                } != null
            }.getOrDefault(false)
            if (!copied) {
                runCatching { resolver.delete(uri, null, null) }
                complete = false
                continue
            }
            val done = ContentValues().apply { put(MediaStore.MediaColumns.IS_PENDING, 0) }
            runCatching { resolver.update(uri, done, null, null) }
        }
        return complete
    }


    private fun moveLegacy(context: Context, source: File, safe: String): Boolean {
        val downloads = Environment.getExternalStoragePublicDirectory(
            Environment.DIRECTORY_DOWNLOADS,
        )
        if (moveTo(File(downloads, FOLDER), source, safe)) return true
        val base = context.getExternalFilesDir(null) ?: context.filesDir
        return moveTo(File(base, FOLDER), source, safe)
    }

    private fun moveTo(folder: File, source: File, safe: String): Boolean {
        if (!folder.exists() && !folder.mkdirs()) return false
        val target = File(folder, safe)
        if (target.exists()) return false
        return source.renameTo(target) || copyTree(source, target)
    }

    private fun copyTree(source: File, target: File): Boolean {
        var complete = true
        source.walkTopDown().forEach { file ->
            val destination = File(target, file.relativeTo(source).path)
            if (file.isDirectory) {
                destination.mkdirs()
                return@forEach
            }
            val copied = runCatching {
                destination.parentFile?.mkdirs()
                file.inputStream().use { input ->
                    destination.outputStream().use { output -> input.copyTo(output, 64 * 1024) }
                }
                true
            }.getOrDefault(false)
            if (!copied) complete = false
        }
        return complete
    }

    private fun safeName(name: String): String {
        val cleaned = name.replace(Regex("[\\\\/:*?\"<>|\\x00-\\x1f]"), "_").trim()
            .trimEnd('.', ' ')
            .take(80)
        return cleaned.ifEmpty { "Torrent" }
    }
}
