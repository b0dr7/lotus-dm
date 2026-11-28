package com.b0dr7.lotusdm.engine

private const val MAX_NAME = 120
private const val MAX_EXT = 8
private val ILLEGAL = charArrayOf('/', '\\', ':', '*', '?', '"', '<', '>', '|')

private fun validExtension(ext: String): Boolean =
    ext.isNotEmpty() && ext.length <= MAX_EXT && ext.all { it.isLetterOrDigit() }

fun hasExtension(name: String): Boolean = validExtension(name.substringAfterLast('.', ""))

fun cleanFileName(input: String, keepExtensionOf: String? = null): String? {
    val trimmed = input.trim().trimEnd('.', ' ')
    if (trimmed.isEmpty()) return null
    if (trimmed.any { it in ILLEGAL || it.code < 32 }) return null

    var name = trimmed
    if (!hasExtension(name)) {
        val ext = keepExtensionOf?.substringAfterLast('.', "") ?: ""
        if (validExtension(ext)) name = "$name.$ext"
    }

    if (name.length > MAX_NAME) {
        val ext = if (hasExtension(name)) name.substringAfterLast('.') else ""
        val stem = if (ext.isEmpty()) name else name.substringBeforeLast('.')
        val room = MAX_NAME - if (ext.isEmpty()) 0 else ext.length + 1
        val cut = stem.take(room).trimEnd('.', ' ')
        if (cut.isEmpty()) return null
        name = if (ext.isEmpty()) cut else "$cut.$ext"
    }

    return name
}
