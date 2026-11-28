package com.b0dr7.lotusdm.ui.screens

import android.Manifest
import android.content.Intent
import android.content.pm.PackageManager
import android.net.Uri
import android.os.Build
import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.result.contract.ActivityResultContracts
import androidx.compose.foundation.BorderStroke
import androidx.compose.foundation.background
import androidx.compose.foundation.horizontalScroll
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.grid.GridCells
import androidx.compose.foundation.lazy.grid.LazyVerticalGrid
import androidx.compose.foundation.lazy.grid.items as gridItems
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.Button
import androidx.compose.material3.ButtonDefaults
import androidx.compose.material3.FilterChip
import androidx.compose.material3.FilterChipDefaults
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.core.content.ContextCompat
import com.b0dr7.lotusdm.engine.DownloadEngine
import com.b0dr7.lotusdm.engine.DownloadJob
import com.b0dr7.lotusdm.engine.JobPriority
import com.b0dr7.lotusdm.engine.JobState
import com.b0dr7.lotusdm.engine.nextTag
import com.b0dr7.lotusdm.service.DownloadForegroundService
import com.b0dr7.lotusdm.ui.Page
import com.b0dr7.lotusdm.ui.rememberWindowClass
import com.b0dr7.lotusdm.ui.theme.LocalLotusColors
import java.io.File

private enum class Filter(val label: String) {
    ALL("All"),
    ACTIVE("Active"),
    DONE("Done"),
    FAILED("Failed"),
}

private val LIMIT_PRESETS = longArrayOf(
    0L,
    256L * 1024L,
    512L * 1024L,
    1024L * 1024L,
    2048L * 1024L,
)

@Composable
fun QueueScreen() {
    val colors = LocalLotusColors.current
    val context = LocalContext.current
    val engine = remember { DownloadEngine.get(context) }

    var jobs by remember { mutableStateOf(engine.snapshot()) }
    var link by remember { mutableStateOf("") }
    var message by remember { mutableStateOf<String?>(null) }
    var filter by remember { mutableStateOf(Filter.ALL) }
    var showBatch by remember { mutableStateOf(false) }
    var batchText by remember { mutableStateOf("") }

    DisposableEffect(engine) {
        engine.observe { updated -> jobs = updated }
        onDispose { engine.observe(null) }
    }

    val legacyStorage = Build.VERSION.SDK_INT < Build.VERSION_CODES.Q
    var writable by remember {
        mutableStateOf(
            !legacyStorage || ContextCompat.checkSelfPermission(
                context,
                Manifest.permission.WRITE_EXTERNAL_STORAGE,
            ) == PackageManager.PERMISSION_GRANTED,
        )
    }
    val permissionLauncher = rememberLauncherForActivityResult(
        ActivityResultContracts.RequestPermission(),
    ) { granted -> writable = granted }

    LaunchedEffect(Unit) {
        if (legacyStorage && !writable) {
            permissionLauncher.launch(Manifest.permission.WRITE_EXTERNAL_STORAGE)
        }
    }

    val torrentPicker = rememberLauncherForActivityResult(
        ActivityResultContracts.OpenDocument(),
    ) { picked ->
        if (picked == null) return@rememberLauncherForActivityResult
        val copied = runCatching {
            val file = File(context.cacheDir, "pick-${System.currentTimeMillis()}.torrent")
            context.contentResolver.openInputStream(picked)?.use { input ->
                file.outputStream().use { output -> input.copyTo(output) }
            } ?: return@runCatching null
            file.takeIf { it.length() > 0L }
        }.getOrNull()
        when {
            copied == null -> message = "Could not read that .torrent file."
            engine.addTorrentFile(copied) == null -> message = "That .torrent file could not be queued."
            else -> DownloadForegroundService.start(context)
        }
    }

    val openJob: (DownloadJob) -> Unit = { job ->
        val uri = job.uri
        if (uri == null) {
            message = "This file is no longer on the phone."
        } else {
            val intent = Intent(Intent.ACTION_VIEW).apply {
                setDataAndType(Uri.parse(uri), job.mime)
                addFlags(Intent.FLAG_GRANT_READ_URI_PERMISSION)
            }
            val opened = runCatching { context.startActivity(intent) }.isSuccess
            if (!opened) message = "No app on this phone can open that file type."
        }
    }

    val visible = jobs.filter { job ->
        when (filter) {
            Filter.ALL -> true
            Filter.ACTIVE -> job.state == JobState.RUNNING ||
                job.state == JobState.QUEUED ||
                job.state == JobState.PAUSED
            Filter.DONE -> job.state == JobState.COMPLETED
            Filter.FAILED -> job.state == JobState.FAILED
        }
    }

    val window = rememberWindowClass()

    val renderJob: @Composable (DownloadJob) -> Unit = { job ->
        JobRow(
            job = job,
            onPause = { engine.pause(job.id) },
            onResume = {
                if (job.state == JobState.FAILED) engine.retry(job.id)
                else engine.resume(job.id)
                DownloadForegroundService.start(context)
            },
            onCancel = { engine.cancel(job.id) },
            onOpen = { openJob(job) },
            onPlay = { engine.play(job.id) },
            onDelete = {
                val removed = engine.delete(job.id)
                if (!removed) message = "Removed the row, but the file was already gone."
            },
            onCycleLimit = { engine.setLimit(job.id, nextLimit(job.limit)) },
            onCycleTag = { engine.setTag(job.id, nextTag(job.tag)) },
            onCyclePriority = { engine.setPriority(job.id, job.priority.next()) },
        )
    }

    Page(window) { pageModifier ->
    Column(
        modifier = pageModifier.padding(horizontal = window.gutter),
    ) {
        Spacer(Modifier.height(20.dp))

        Row(verticalAlignment = Alignment.CenterVertically) {
            Text(
                text = "Queue",
                color = colors.text,
                fontSize = 26.sp,
                fontWeight = FontWeight.SemiBold,
            )
            Spacer(Modifier.weight(1f))
            TextButton(onClick = { showBatch = true }) { Text("Batch") }
            if (jobs.any { it.state == JobState.COMPLETED }) {
                TextButton(onClick = { engine.clearCompleted() }) { Text("Clear done") }
            }
        }

        Spacer(Modifier.height(8.dp))

        Row(
            modifier = Modifier.horizontalScroll(rememberScrollState()),
            horizontalArrangement = Arrangement.spacedBy(8.dp),
        ) {
            Filter.entries.forEach { candidate ->
                FilterChip(
                    selected = filter == candidate,
                    onClick = { filter = candidate },
                    label = { Text(candidate.label, fontSize = 12.sp) },
                    shape = RoundedCornerShape(999.dp),
                    colors = FilterChipDefaults.filterChipColors(
                        selectedContainerColor = colors.accent.copy(alpha = 0.16f),
                        selectedLabelColor = colors.accent,
                        labelColor = colors.muted,
                    ),
                )
            }
        }

        Spacer(Modifier.height(12.dp))

        Row(verticalAlignment = Alignment.CenterVertically) {
            OutlinedTextField(
                value = link,
                onValueChange = {
                    link = it
                    message = null
                },
                modifier = Modifier.weight(1f),
                singleLine = true,
                placeholder = { Text("https://… or magnet:?xt=…", fontSize = 13.sp) },
                keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Uri),
            )
            Spacer(Modifier.width(8.dp))
            Button(
                onClick = {
                    val job = engine.add(link)
                    if (job == null) {
                        message = "That is neither an http(s) link nor a magnet."
                    } else {
                        link = ""
                        DownloadForegroundService.start(context)
                    }
                },
                shape = RoundedCornerShape(12.dp),
                colors = ButtonDefaults.buttonColors(
                    containerColor = colors.accent,
                    contentColor = colors.onAccent,
                ),
            ) {
                Text("Add", fontWeight = FontWeight.SemiBold)
            }
        }

        TextButton(
            onClick = {
                torrentPicker.launch(arrayOf("application/x-bittorrent", "application/octet-stream"))
            },
        ) {
            Text("Add .torrent file", fontSize = 12.sp)
        }

        message?.let { text ->
            Spacer(Modifier.height(8.dp))
            Text(text = text, color = colors.muted, fontSize = 12.sp)
        }

        Spacer(Modifier.height(16.dp))

        if (visible.isEmpty()) {
            EmptyQueue()
        } else if (window.isMultiColumn) {
            LazyVerticalGrid(
                columns = GridCells.Adaptive(minSize = 340.dp),
                modifier = Modifier.fillMaxSize(),
                horizontalArrangement = Arrangement.spacedBy(10.dp),
                verticalArrangement = Arrangement.spacedBy(10.dp),
            ) {
                gridItems(visible, key = { it.id }) { job -> renderJob(job) }
            }
        } else {
            LazyColumn(
                modifier = Modifier.fillMaxSize(),
                verticalArrangement = Arrangement.spacedBy(8.dp),
            ) {
                items(visible, key = { it.id }) { job -> renderJob(job) }
                item { Spacer(Modifier.height(16.dp)) }
            }
        }
    }
    }

    if (showBatch) {
        AlertDialog(
            onDismissRequest = { showBatch = false },
            title = { Text("Paste links") },
            text = {
                OutlinedTextField(
                    value = batchText,
                    onValueChange = { batchText = it },
                    modifier = Modifier.fillMaxWidth(),
                    minLines = 5,
                    placeholder = { Text("One link per line", fontSize = 13.sp) },
                    shape = RoundedCornerShape(12.dp),
                )
            },
            confirmButton = {
                TextButton(onClick = {
                    val lines = batchText.split('\n')
                        .map { it.trim() }
                        .filter {
                            it.startsWith("http://") || it.startsWith("https://") ||
                                it.startsWith("magnet:")
                        }
                    lines.forEach { engine.add(it) }
                    if (lines.isNotEmpty()) DownloadForegroundService.start(context)
                    message = if (lines.isEmpty()) {
                        "No links found in that text."
                    } else {
                        "Added ${lines.size} download${if (lines.size == 1) "" else "s"}."
                    }
                    batchText = ""
                    showBatch = false
                }) { Text("Add all") }
            },
            dismissButton = {
                TextButton(onClick = { showBatch = false }) { Text("Cancel") }
            },
        )
    }
}

@Composable
private fun EmptyQueue() {
    val colors = LocalLotusColors.current
    Box(
        modifier = Modifier.fillMaxSize().padding(horizontal = 12.dp),
        contentAlignment = Alignment.Center,
    ) {
        Column(horizontalAlignment = Alignment.CenterHorizontally) {
            Text(
                text = "Nothing here",
                color = colors.text,
                fontSize = 18.sp,
                fontWeight = FontWeight.SemiBold,
            )
            Spacer(Modifier.height(8.dp))
            Text(
                text = "Paste a direct link above. Finished files land in Downloads/LotusDM.",
                color = colors.muted,
                fontSize = 13.sp,
                textAlign = TextAlign.Center,
            )
        }
    }
}

@Composable
private fun JobRow(
    job: DownloadJob,
    onPause: () -> Unit,
    onResume: () -> Unit,
    onCancel: () -> Unit,
    onOpen: () -> Unit,
    onPlay: () -> Unit,
    onDelete: () -> Unit,
    onCycleLimit: () -> Unit,
    onCycleTag: () -> Unit,
    onCyclePriority: () -> Unit,
) {
    val colors = LocalLotusColors.current

    Surface(
        modifier = Modifier.fillMaxWidth(),
        shape = RoundedCornerShape(14.dp),
        color = colors.surface,
        border = BorderStroke(1.dp, colors.border),
    ) {
        Column(modifier = Modifier.padding(14.dp)) {
            Row(verticalAlignment = Alignment.CenterVertically) {
                Text(
                    text = job.fileName,
                    color = colors.text,
                    fontSize = 14.sp,
                    fontWeight = FontWeight.Medium,
                    maxLines = 1,
                    overflow = TextOverflow.Ellipsis,
                    modifier = Modifier.weight(1f),
                )
                if (job.tag.isNotEmpty()) {
                    Spacer(Modifier.width(8.dp))
                    Badge(text = job.tag, accent = false)
                }
                if (job.priority != JobPriority.NORMAL) {
                    Spacer(Modifier.width(6.dp))
                    Badge(text = job.priority.label, accent = true)
                }
            }
            Spacer(Modifier.height(4.dp))
            Text(
                text = metaLine(job),
                color = if (job.state == JobState.FAILED) colors.accent else colors.muted,
                fontSize = 12.sp,
                maxLines = 1,
                overflow = TextOverflow.Ellipsis,
            )
            Spacer(Modifier.height(10.dp))

            Box(
                modifier = Modifier
                    .fillMaxWidth()
                    .height(6.dp)
                    .clip(RoundedCornerShape(3.dp))
                    .background(colors.surfaceAlt),
            ) {
                Box(
                    modifier = Modifier
                        .fillMaxWidth(job.progress)
                        .height(6.dp)
                        .clip(RoundedCornerShape(3.dp))
                        .background(colors.accent),
                )
            }

            Spacer(Modifier.height(10.dp))

            Row(
                verticalAlignment = Alignment.CenterVertically,
                horizontalArrangement = Arrangement.spacedBy(4.dp),
            ) {
                when (job.state) {
                    JobState.RUNNING -> TextButton(onClick = onPause) { Text("Pause") }
                    JobState.PAUSED -> TextButton(onClick = onResume) { Text("Resume") }
                    JobState.FAILED -> TextButton(onClick = onResume) { Text("Retry") }
                    JobState.QUEUED -> TextButton(onClick = onResume) { Text("Start") }
                    JobState.COMPLETED -> {
                        if (job.isMedia) {
                            TextButton(onClick = onPlay) { Text("Play") }
                        }
                        TextButton(onClick = onOpen, enabled = job.uri != null) { Text("Open") }
                        TextButton(onClick = onDelete) { Text("Delete") }
                    }
                }

                if (job.state != JobState.COMPLETED) {
                    TextButton(onClick = onCancel) { Text("Cancel") }
                    TextButton(onClick = onCycleLimit) { Text(limitLabel(job.limit)) }
                }
            }

            Spacer(Modifier.height(2.dp))
            Row(
                verticalAlignment = Alignment.CenterVertically,
                horizontalArrangement = Arrangement.spacedBy(4.dp),
            ) {
                TextButton(onClick = onCycleTag) {
                    Text(if (job.tag.isEmpty()) "+ Tag" else "Tag: ${job.tag}")
                }
                TextButton(onClick = onCyclePriority) {
                    Text("Priority: ${job.priority.label}")
                }
            }
        }
    }
}

@Composable
private fun Badge(text: String, accent: Boolean) {
    val colors = LocalLotusColors.current
    Surface(
        shape = RoundedCornerShape(999.dp),
        color = if (accent) colors.accent.copy(alpha = 0.16f) else colors.surfaceAlt,
    ) {
        Text(
            text = text,
            color = if (accent) colors.accent else colors.muted,
            fontSize = 11.sp,
            modifier = Modifier.padding(horizontal = 8.dp, vertical = 2.dp),
        )
    }
}

private fun nextLimit(current: Long): Long {
    val index = LIMIT_PRESETS.indexOf(current)
    return LIMIT_PRESETS[(index + 1) % LIMIT_PRESETS.size]
}

private fun limitLabel(limit: Long): String =
    if (limit <= 0L) "No limit" else "${limit / 1024} KB/s"

private fun metaLine(job: DownloadJob): String {
    val parts = mutableListOf<String>()
    if (job.awaitingWifi) {
        parts += "Waiting for Wi-Fi"
    } else {
        if (job.isTorrent) parts += "Torrent"
        if (job.isMedia && job.quality != null) parts += job.quality!!
        parts += when (job.state) {
            JobState.RUNNING -> "Downloading"
            JobState.QUEUED -> job.error ?: "Queued"
            JobState.PAUSED -> "Paused"
            JobState.FAILED -> job.error ?: "Failed"
            JobState.COMPLETED -> job.error ?: "In Downloads/LotusDM"
        }
    }
    parts += if (job.total > 0L) {
        "${formatBytes(job.downloaded)} of ${formatBytes(job.total)}"
    } else {
        formatBytes(job.downloaded)
    }
    if (job.state == JobState.RUNNING && job.speed > 0L) {
        parts += "${formatBytes(job.speed)}/s"
    }
    return parts.joinToString(" - ")
}

private fun formatBytes(bytes: Long): String {
    if (bytes <= 0L) return "0 B"
    val units = arrayOf("B", "KB", "MB", "GB", "TB")
    var value = bytes.toDouble()
    var unit = 0
    while (value >= 1024 && unit < units.lastIndex) {
        value /= 1024
        unit++
    }
    return if (unit == 0) "$bytes B" else String.format("%.1f %s", value, units[unit])
}
