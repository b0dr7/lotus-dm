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
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.material3.Button
import androidx.compose.material3.ButtonDefaults
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
import com.b0dr7.lotusdm.engine.JobState
import com.b0dr7.lotusdm.service.DownloadForegroundService
import com.b0dr7.lotusdm.ui.theme.LocalLotusColors

@Composable
fun QueueScreen() {
    val colors = LocalLotusColors.current
    val context = LocalContext.current
    val engine = remember { DownloadEngine.get(context) }

    var jobs by remember { mutableStateOf(engine.snapshot()) }
    var link by remember { mutableStateOf("") }
    var message by remember { mutableStateOf<String?>(null) }

    DisposableEffect(engine) {
        engine.observe { updated -> jobs = updated }
        onDispose { engine.observe(null) }
    }

    // API 23-28 need the write permission before we can use public Downloads.
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

    Column(
        modifier = Modifier.fillMaxSize().padding(horizontal = 20.dp),
    ) {
        Spacer(Modifier.height(24.dp))
        Text(
            text = "Queue",
            color = colors.text,
            fontSize = 26.sp,
            fontWeight = FontWeight.SemiBold,
        )
        Spacer(Modifier.height(16.dp))

        Row(verticalAlignment = Alignment.CenterVertically) {
            OutlinedTextField(
                value = link,
                onValueChange = {
                    link = it
                    message = null
                },
                modifier = Modifier.weight(1f),
                singleLine = true,
                placeholder = { Text("https://example.com/file.zip", fontSize = 13.sp) },
                shape = RoundedCornerShape(12.dp),
                keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Uri),
            )
            Spacer(Modifier.width(8.dp))
            Button(
                onClick = {
                    val job = engine.add(link)
                    if (job == null) {
                        message = "That does not look like an http(s) link."
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

        message?.let { text ->
            Spacer(Modifier.height(8.dp))
            Text(text = text, color = colors.muted, fontSize = 12.sp)
        }

        Spacer(Modifier.height(16.dp))

        if (jobs.isEmpty()) {
            EmptyQueue()
        } else {
            LazyColumn(
                modifier = Modifier.fillMaxSize(),
                verticalArrangement = Arrangement.spacedBy(8.dp),
            ) {
                items(jobs, key = { it.id }) { job ->
                    JobRow(
                        job = job,
                        onPause = { engine.pause(job.id) },
                        onResume = {
                            engine.resume(job.id)
                            DownloadForegroundService.start(context)
                        },
                        onCancel = { engine.cancel(job.id) },
                        onOpen = { openJob(job) },
                        onDelete = {
                            val removed = engine.delete(job.id)
                            if (!removed) message = "Removed the row, but the file was already gone."
                        },
                    )
                }
                item { Spacer(Modifier.height(16.dp)) }
            }
        }
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
                text = "Nothing queued",
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
    onDelete: () -> Unit,
) {
    val colors = LocalLotusColors.current

    Surface(
        modifier = Modifier.fillMaxWidth(),
        shape = RoundedCornerShape(14.dp),
        color = colors.surface,
        border = BorderStroke(1.dp, colors.border),
    ) {
        Column(modifier = Modifier.padding(14.dp)) {
            Text(
                text = job.fileName,
                color = colors.text,
                fontSize = 14.sp,
                fontWeight = FontWeight.Medium,
                maxLines = 1,
                overflow = TextOverflow.Ellipsis,
            )
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
                    JobState.PAUSED, JobState.FAILED -> TextButton(onClick = onResume) {
                        Text(if (job.state == JobState.FAILED) "Retry" else "Resume")
                    }
                    JobState.QUEUED -> TextButton(onClick = onResume) { Text("Start") }
                    JobState.COMPLETED -> {
                        TextButton(onClick = onOpen, enabled = job.uri != null) { Text("Open") }
                        TextButton(onClick = onDelete) { Text("Delete") }
                    }
                }

                if (job.state != JobState.COMPLETED) {
                    TextButton(onClick = onCancel) { Text("Cancel") }
                }
            }
        }
    }
}

private fun metaLine(job: DownloadJob): String {
    val parts = mutableListOf<String>()
    parts += when (job.state) {
        JobState.RUNNING -> "Downloading"
        JobState.QUEUED -> "Queued"
        JobState.PAUSED -> "Paused"
        JobState.FAILED -> job.error ?: "Failed"
        JobState.COMPLETED -> job.error ?: "In Downloads/LotusDM"
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
