package com.b0dr7.lotusdm.engine

enum class JobState {
    QUEUED,
    RUNNING,
    PAUSED,
    COMPLETED,
    FAILED,
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
) {
    val progress: Float
        get() = if (total > 0L) (downloaded.toFloat() / total.toFloat()).coerceIn(0f, 1f) else 0f
}
