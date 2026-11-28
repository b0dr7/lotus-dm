package com.b0dr7.lotusdm.engine

import android.Manifest
import android.content.ContentValues
import android.content.Context
import android.content.pm.PackageManager
import android.net.Uri
import android.os.Build
import android.os.Environment
import android.provider.MediaStore
import android.webkit.MimeTypeMap
import androidx.core.content.ContextCompat
import androidx.core.content.FileProvider
import java.io.File

data class PublishedFile(
    val uri: String,
    val path: String?,
    val displayName: String,
)

object CompletedFileStore {

    private const val RELATIVE_PATH = "Download/LotusDM"
    private const val FOLDER = "LotusDM"
    private const val FALLBACK_EXTRA = "LotusDM"

    fun mimeFor(fileName: String): String {
        val extension = fileName.substringAfterLast('.', "").lowercase()
        if (extension.isEmpty()) return "application/octet-stream"
        return MimeTypeMap.getSingleton().getMimeTypeFromExtension(extension)
            ?: "application/octet-stream"
    }

    fun canWritePublic(context: Context): Boolean {
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.Q) return true
        val granted = ContextCompat.checkSelfPermission(
            context,
            Manifest.permission.WRITE_EXTERNAL_STORAGE,
        ) == PackageManager.PERMISSION_GRANTED
        return granted
    }

    fun publish(context: Context, source: File, desiredName: String): PublishedFile? {
        if (!source.exists()) return null

        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.Q) {
            publishToMediaStore(context, source, desiredName)?.let { return it }
        } else if (canWritePublic(context)) {
            publishToLegacyDownloads(context, source, desiredName)?.let { return it }
        }
        return publishToPrivateStorage(context, source, desiredName)
    }

    fun rename(
        context: Context,
        uri: String?,
        path: String?,
        newName: String,
    ): PublishedFile? {
        if (path != null) {
            val source = File(path)
            if (source.exists()) {
                val target = File(source.parentFile, newName)
                val clash = target.exists() && target.absolutePath != source.absolutePath
                if (!clash && runCatching { source.renameTo(target) }.getOrDefault(false)) {
                    return PublishedFile(providerUri(context, target), target.absolutePath, newName)
                }
            }
        }
        if (uri != null) {
            val parsed = runCatching { Uri.parse(uri) }.getOrNull() ?: return null
            if (parsed.scheme != "content") return null
            val values = ContentValues().apply {
                put(MediaStore.MediaColumns.DISPLAY_NAME, newName)
                put(MediaStore.MediaColumns.MIME_TYPE, mimeFor(newName))
            }
            val updated = runCatching {
                context.contentResolver.update(parsed, values, null, null)
            }.getOrDefault(0)
            if (updated > 0) return PublishedFile(uri, null, newName)
        }
        return null
    }

    fun delete(context: Context, uri: String, path: String?): Boolean {
        val parsed = runCatching { Uri.parse(uri) }.getOrNull()
        if (parsed != null && parsed.scheme == "content") {
            val removed = runCatching {
                context.contentResolver.delete(parsed, null, null)
            }.getOrDefault(0)
            if (removed > 0) return true
        }
        if (path != null) {
            return runCatching { File(path).delete() }.getOrDefault(false)
        }
        return false
    }


    private fun publishToMediaStore(
        context: Context,
        source: File,
        desiredName: String,
    ): PublishedFile? {
        val resolver = context.contentResolver
        val collection = MediaStore.Downloads.EXTERNAL_CONTENT_URI
        val name = uniqueName(desiredName, existingMediaStoreNames(context))

        val values = ContentValues().apply {
            put(MediaStore.MediaColumns.DISPLAY_NAME, name)
            put(MediaStore.MediaColumns.MIME_TYPE, mimeFor(name))
            put(MediaStore.MediaColumns.RELATIVE_PATH, RELATIVE_PATH)
            put(MediaStore.MediaColumns.IS_PENDING, 1)
        }

        val uri = runCatching { resolver.insert(collection, values) }.getOrNull() ?: return null

        val copied = runCatching {
            resolver.openOutputStream(uri)?.use { output ->
                source.inputStream().use { input -> input.copyTo(output, 64 * 1024) }
            } != null
        }.getOrDefault(false)

        if (!copied) {
            runCatching { resolver.delete(uri, null, null) }
            return null
        }

        val done = ContentValues().apply { put(MediaStore.MediaColumns.IS_PENDING, 0) }
        runCatching { resolver.update(uri, done, null, null) }
        source.delete()

        return PublishedFile(uri.toString(), null, name)
    }

    private fun existingMediaStoreNames(context: Context): Set<String> {
        val names = mutableSetOf<String>()
        runCatching {
            context.contentResolver.query(
                MediaStore.Downloads.EXTERNAL_CONTENT_URI,
                arrayOf(MediaStore.MediaColumns.DISPLAY_NAME),
                "${MediaStore.MediaColumns.RELATIVE_PATH} LIKE ?",
                arrayOf("$RELATIVE_PATH%"),
                null,
            )?.use { cursor ->
                while (cursor.moveToNext()) {
                    cursor.getString(0)?.let { names.add(it) }
                }
            }
        }
        return names
    }


    private fun publishToLegacyDownloads(
        context: Context,
        source: File,
        desiredName: String,
    ): PublishedFile? {
        val folder = File(
            Environment.getExternalStoragePublicDirectory(Environment.DIRECTORY_DOWNLOADS),
            FOLDER,
        )
        if (!folder.exists() && !folder.mkdirs()) return null

        val existing = folder.list()?.toSet().orEmpty()
        val name = uniqueName(desiredName, existing)
        val target = File(folder, name)

        val copied = runCatching {
            source.inputStream().use { input ->
                target.outputStream().use { output -> input.copyTo(output, 64 * 1024) }
            }
            true
        }.getOrDefault(false)

        if (!copied) return null
        source.delete()

        return PublishedFile(
            uri = providerUri(context, target),
            path = target.absolutePath,
            displayName = name,
        )
    }


    private fun publishToPrivateStorage(
        context: Context,
        source: File,
        desiredName: String,
    ): PublishedFile? {
        val base = context.getExternalFilesDir(null) ?: context.filesDir
        val folder = File(base, FALLBACK_EXTRA)
        if (!folder.exists() && !folder.mkdirs()) return null

        val name = uniqueName(desiredName, folder.list()?.toSet().orEmpty())
        val target = File(folder, name)

        val moved = source.renameTo(target) || runCatching {
            source.inputStream().use { input ->
                target.outputStream().use { output -> input.copyTo(output, 64 * 1024) }
            }
            source.delete()
            true
        }.getOrDefault(false)

        if (!moved) return null

        return PublishedFile(
            uri = providerUri(context, target),
            path = target.absolutePath,
            displayName = name,
        )
    }

    private fun providerUri(context: Context, file: File): String =
        runCatching {
            FileProvider.getUriForFile(context, "${context.packageName}.files", file).toString()
        }.getOrElse { Uri.fromFile(file).toString() }

    private fun uniqueName(desired: String, existing: Set<String>): String {
        if (desired !in existing) return desired

        val stem = desired.substringBeforeLast('.', desired)
        val extension = if (desired.contains('.')) "." + desired.substringAfterLast('.') else ""

        var index = 1
        while (true) {
            val candidate = "$stem ($index)$extension"
            if (candidate !in existing) return candidate
            index++
        }
    }
}
