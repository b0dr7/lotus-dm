package com.b0dr7.lotusdm.engine

enum class JobState {
    QUEUED,
    RUNNING,
    PAUSED,
    COMPLETED,
    FAILED,
}

enum class JobKind {
    HTTP,
    TORRENT,
}

enum class JobPriority(val label: String, val rank: Int) {
    HIGH("High", 2),
    NORMAL("Normal", 1),
    LOW("Low", 0),
    ;

    fun next(): JobPriority = when (this) {
        HIGH -> NORMAL
        NORMAL -> LOW
        LOW -> HIGH
    }
}

data class DownloadJob(
    val id: String,
    val url: String,
    val fileName: String,
    val state: JobState,
    val downloaded: Long,
    val total: Long,
    val speed: Long,
    val error: String?,
    val uri: String? = null,
    val mime: String = "application/octet-stream",
    val limit: Long = 0L,
    val tag: String = "",
    val priority: JobPriority = JobPriority.NORMAL,
    val kind: JobKind = JobKind.HTTP,
    val awaitingWifi: Boolean = false,
    val quality: String? = null,
    val mirrors: List<String> = emptyList(),
    val startAt: Long = 0L,
) {
    val progress: Float
        get() = if (total > 0L) (downloaded.toFloat() / total.toFloat()).coerceIn(0f, 1f) else 0f

    val isTorrent: Boolean get() = kind == JobKind.TORRENT

    val isMedia: Boolean
        get() = isMediaUrl(url) && kind == JobKind.HTTP && quality != null
}

val JOB_TAGS = listOf("Video", "Music", "Docs", "Apps", "Archives", "Other")

fun nextTag(current: String): String {
    if (current.isEmpty()) return JOB_TAGS.first()
    val index = JOB_TAGS.indexOf(current)
    if (index < 0 || index == JOB_TAGS.lastIndex) return ""
    return JOB_TAGS[index + 1]
}
