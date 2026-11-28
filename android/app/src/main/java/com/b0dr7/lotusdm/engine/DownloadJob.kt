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

/** Queue order: higher rank starts first. */
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

/**
 * What the app is allowed to see of a download. Plain data - the engine
 * owns the mutable copy.
 */
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
    /** True while the Wi-Fi-only gate is holding this job. */
    val awaitingWifi: Boolean = false,
    /** Media pick for video/audio jobs: a height like "1080p" or a format like "mp3". */
    val quality: String? = null,
) {
    val progress: Float
        get() = if (total > 0L) (downloaded.toFloat() / total.toFloat()).coerceIn(0f, 1f) else 0f

    val isTorrent: Boolean get() = kind == JobKind.TORRENT

    /** True for media jobs (video/audio picks) the queue plays back. */
    val isMedia: Boolean
        get() = isMediaUrl(url) && kind == JobKind.HTTP && quality != null
}

/** The tags the app offers; an empty tag means "untagged". */
val JOB_TAGS = listOf("Video", "Music", "Docs", "Apps", "Archives", "Other")

fun nextTag(current: String): String {
    if (current.isEmpty()) return JOB_TAGS.first()
    val index = JOB_TAGS.indexOf(current)
    if (index < 0 || index == JOB_TAGS.lastIndex) return ""
    return JOB_TAGS[index + 1]
}
