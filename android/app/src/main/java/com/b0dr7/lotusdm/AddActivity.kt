package com.b0dr7.lotusdm

import android.content.Context
import android.content.Intent
import android.net.Uri
import android.os.Bundle
import android.widget.Toast
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
import androidx.compose.foundation.clickable
import android.graphics.BitmapFactory
import androidx.compose.foundation.Image
import androidx.compose.runtime.LaunchedEffect
import kotlinx.coroutines.withContext
import androidx.compose.ui.graphics.asImageBitmap
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.imePadding
import androidx.compose.foundation.layout.navigationBarsPadding
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.BottomSheetDefaults
import androidx.compose.material3.Button
import androidx.compose.material3.ButtonDefaults
import androidx.compose.material3.Checkbox
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.ModalBottomSheet
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.OutlinedTextFieldDefaults
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.material3.rememberModalBottomSheetState
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import com.b0dr7.lotusdm.data.SettingsStore
import com.b0dr7.lotusdm.engine.DownloadEngine
import com.b0dr7.lotusdm.engine.isAudioQuality
import com.b0dr7.lotusdm.engine.isMediaUrl
import com.b0dr7.lotusdm.service.DownloadForegroundService
import com.b0dr7.lotusdm.ui.theme.LocalLotusColors
import com.b0dr7.lotusdm.ui.theme.LotusTheme
import java.io.File

/**
 * Receives links from browsers (ACTION_VIEW) and the share sheet
 * (ACTION_SEND) and asks before downloading: a bottom sheet with the link and
 * the file name, in the style of AB Download Manager's "New download".
 */
private const val MIME_TORRENT = "application/x-bittorrent"
private val VIDEO_QUALITIES = listOf("2160p", "1440p", "1080p", "720p", "480p", "360p", "240p")
private val AUDIO_FORMATS = listOf("mp3", "aac", "opus", "flac", "wav")
private const val DEFAULT_VIDEO_QUALITY = "720p"

class AddActivity : ComponentActivity() {

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)

        val link = extractLink(intent)
        val mime = intent?.type
        val torrent = intent?.data?.takeIf { mime == MIME_TORRENT }

        setContent {
            val store = remember { SettingsStore(applicationContext) }
            LotusTheme(mode = store.themeMode) {
                NewDownloadSheet(
                    incoming = link ?: torrent?.toString(),
                    mime = mime,
                    onConfirm = { url, name, quality, startAt ->
                        val queued = if (torrent != null) {
                            val copy = copyTorrent(applicationContext, torrent)
                            copy?.let { DownloadEngine.get(applicationContext).addTorrentFile(it) }
                        } else {
                            DownloadEngine.get(applicationContext).add(url, name, quality, startAt)
                        }
                        if (queued != null) {
                            DownloadForegroundService.start(applicationContext)
                        }
                        Toast.makeText(
                            applicationContext,
                            if (queued != null) "Added to the queue" else "That link was not usable",
                            Toast.LENGTH_SHORT,
                        ).show()
                        finish()
                    },
                    onCancel = { finish() },
                )
            }
        }
    }
}

@OptIn(ExperimentalMaterial3Api::class)
@Composable
private fun NewDownloadSheet(
    incoming: String?,
    mime: String?,
    onConfirm: (String, String?, String?, Long) -> Unit,
    onCancel: () -> Unit,
) {
    val colors = LocalLotusColors.current
    val state = rememberModalBottomSheetState(skipPartiallyExpanded = true)

    var link by remember(incoming) { mutableStateOf(incoming.orEmpty()) }
    var name by remember(incoming) { mutableStateOf(fileNameOf(incoming).orEmpty()) }
    // Until the name is edited by hand it follows the link, like AB DM does.
    var nameTouched by remember(incoming) { mutableStateOf(false) }
    // Media quality for recognised media shares; the default is the FDM-ish
    // middle pick until the user chooses.
    var quality by remember(incoming) { mutableStateOf(DEFAULT_VIDEO_QUALITY) }
    var delay by remember(incoming) { mutableStateOf(false) }
    var startIn by remember(incoming) { mutableStateOf("") }

    val media = incoming != null && isMediaUrl(incoming)

    val usable = isDownloadable(link.trim())

    ModalBottomSheet(
        onDismissRequest = onCancel,
        sheetState = state,
        containerColor = colors.surface,
        contentColor = colors.text,
        scrimColor = androidx.compose.ui.graphics.Color.Black.copy(alpha = 0.55f),
        tonalElevation = 0.dp,
        shape = RoundedCornerShape(topStart = 24.dp, topEnd = 24.dp),
        dragHandle = {
            BottomSheetDefaults.DragHandle(
                width = 38.dp,
                color = colors.border,
            )
        },
    ) {
        Column(
            modifier = Modifier
                .fillMaxWidth()
                .imePadding()
                .navigationBarsPadding()
                .padding(horizontal = 20.dp)
                .padding(bottom = 18.dp),
        ) {
            Row(verticalAlignment = Alignment.CenterVertically) {
                Text(
                    text = "New download",
                    color = colors.text,
                    fontSize = 20.sp,
                    fontWeight = FontWeight.SemiBold,
                )
                Spacer(Modifier.weight(1f))
                TextButton(onClick = onCancel) {
                    Text("Close", color = colors.muted, fontSize = 13.sp)
                }
            }
            Text(
                text = if (incoming != null) {
                    "Sent here by another app. Nothing downloads until you press Download."
                } else {
                    "That share did not contain a link or a .torrent file."
                },
                color = colors.muted,
                fontSize = 12.5.sp,
            )

            val thumbUrl = youtubeThumbnailUrl(link)
            if (thumbUrl != null) {
                Spacer(Modifier.height(12.dp))
                var bitmap by remember { mutableStateOf<android.graphics.Bitmap?>(null) }
                LaunchedEffect(thumbUrl) {
                    bitmap = withContext(kotlinx.coroutines.Dispatchers.IO) {
                        runCatching {
                            val conn = java.net.URL(thumbUrl).openConnection() as java.net.HttpURLConnection
                            conn.doInput = true
                            conn.connect()
                            BitmapFactory.decodeStream(conn.inputStream)
                        }.getOrNull()
                    }
                }
                bitmap?.let { bmp ->
                    Image(
                        bitmap = bmp.asImageBitmap(),
                        contentDescription = "Video thumbnail",
                        modifier = Modifier
                            .fillMaxWidth()
                            .height(180.dp)
                            .clip(RoundedCornerShape(12.dp)),
                        contentScale = androidx.compose.ui.layout.ContentScale.Crop,
                    )
                }
            }

            Spacer(Modifier.height(16.dp))

            Column(
                modifier = Modifier
                    .weight(1f, fill = false)
                    .verticalScroll(rememberScrollState()),
                verticalArrangement = Arrangement.spacedBy(12.dp),
            ) {
                OutlinedTextField(
                    value = link,
                    onValueChange = {
                        link = it
                        if (!nameTouched) name = fileNameOf(it).orEmpty()
                    },
                    modifier = Modifier.fillMaxWidth(),
                    label = { Text("Link", fontSize = 12.sp) },
                    singleLine = true,
                    maxLines = 1,
                    shape = RoundedCornerShape(12.dp),
                    colors = lotusFieldColors(colors.accent, colors.border, colors.muted, colors.text),
                )
                OutlinedTextField(
                    value = name,
                    onValueChange = {
                        name = it
                        nameTouched = true
                    },
                    modifier = Modifier.fillMaxWidth(),
                    label = { Text("Name", fontSize = 12.sp) },
                    singleLine = true,
                    maxLines = 1,
                    shape = RoundedCornerShape(12.dp),
                    colors = lotusFieldColors(colors.accent, colors.border, colors.muted, colors.text),
                )
                Row(verticalAlignment = Alignment.CenterVertically) {
                    Text(
                        text = "Saves to Downloads/LotusDM",
                        color = colors.muted,
                        fontSize = 12.sp,
                    )
                    if (mime != null) {
                        Spacer(Modifier.weight(1f))
                        Text(
                            text = mime,
                            color = colors.muted,
                            fontSize = 11.sp,
                            maxLines = 1,
                            overflow = TextOverflow.Ellipsis,
                        )
                    }
                }

                if (media) {
                    Spacer(Modifier.height(18.dp))
                    Text(
                        text = if (isAudioQuality(quality)) "Audio format" else "Video quality",
                        color = colors.text,
                        fontSize = 12.5.sp,
                        fontWeight = FontWeight.SemiBold,
                    )
                    Spacer(Modifier.height(8.dp))
                    Column(verticalArrangement = Arrangement.spacedBy(6.dp)) {
                        (if (isAudioQuality(quality)) AUDIO_FORMATS else VIDEO_QUALITIES)
                            .forEach { candidate ->
                                QualityRow(
                                    label = candidate,
                                    selected = quality == candidate,
                                    onClick = { quality = candidate },
                                    accent = colors.accent,
                                    text = colors.text,
                                    muted = colors.muted,
                                )
                            }
                    }
                    Spacer(Modifier.height(10.dp))
                }
            }

            Spacer(Modifier.height(18.dp))

            Row(verticalAlignment = Alignment.CenterVertically) {
                Checkbox(checked = delay, onCheckedChange = { delay = it })
                Text("Delay start", color = colors.text, fontSize = 12.5.sp)
                if (delay) {
                    Spacer(Modifier.width(8.dp))
                    OutlinedTextField(
                        value = startIn,
                        onValueChange = { startIn = it },
                        modifier = Modifier.width(72.dp),
                        placeholder = { Text("min", fontSize = 11.sp) },
                        singleLine = true,
                        shape = RoundedCornerShape(12.dp),
                        colors = lotusFieldColors(colors.accent, colors.border, colors.muted, colors.text),
                    )
                }
            }

            Button(
                onClick = {
                    val startAt = if (delay) {
                        startIn.trim().toLongOrNull()?.let { minutes ->
                            System.currentTimeMillis() + minutes * 60_000L
                        } ?: 0L
                    } else 0L
                    onConfirm(
                        link.trim(),
                        name.trim().takeIf { it.isNotEmpty() },
                        if (media) quality else null,
                        startAt,
                    )
                },
                enabled = usable,
                modifier = Modifier.fillMaxWidth(),
                shape = RoundedCornerShape(12.dp),
                colors = ButtonDefaults.buttonColors(
                    containerColor = colors.accent,
                    contentColor = colors.onAccent,
                ),
            ) {
                Text(if (usable) "Download" else "Nothing to download", fontWeight = FontWeight.SemiBold)
            }

            TextButton(onClick = onCancel, modifier = Modifier.fillMaxWidth()) {
                Text("Cancel", color = colors.muted, fontSize = 13.sp)
            }
        }
    }
}

@Composable
private fun lotusFieldColors(
    accent: androidx.compose.ui.graphics.Color,
    border: androidx.compose.ui.graphics.Color,
    muted: androidx.compose.ui.graphics.Color,
    text: androidx.compose.ui.graphics.Color,
) = OutlinedTextFieldDefaults.colors(
    focusedBorderColor = accent,
    unfocusedBorderColor = border,
    focusedLabelColor = accent,
    unfocusedLabelColor = muted,
    cursorColor = accent,
    focusedTextColor = text,
    unfocusedTextColor = text,
)

@Composable
private fun QualityRow(
    label: String,
    selected: Boolean,
    onClick: () -> Unit,
    accent: androidx.compose.ui.graphics.Color,
    text: androidx.compose.ui.graphics.Color,
    muted: androidx.compose.ui.graphics.Color,
) {
    Row(
        verticalAlignment = Alignment.CenterVertically,
        modifier = Modifier
            .fillMaxWidth()
            .clickable { onClick }
            .padding(vertical = 10.dp),
    ) {
        Text(
            text = label,
            modifier = Modifier.weight(1f),
            color = if (selected) accent else muted,
            fontSize = 13.5.sp,
            fontWeight = if (selected) FontWeight.SemiBold else FontWeight.Normal,
        )
    }
}

private fun youtubeVideoId(url: String): String? {
    val lower = url.lowercase()
    if (!lower.contains("youtube.com") && !lower.contains("youtu.be")) return null
    // youtu.be/VIDEO_ID
    Regex("youtu\\.be/([A-Za-z0-9_-]{11})").find(url)?.groupValues?.get(1)?.let { return it }
    // youtube.com/shorts/VIDEO_ID
    Regex("youtube\\.com/shorts/([A-Za-z0-9_-]{11})").find(url)?.groupValues?.get(1)?.let { return it }
    // youtube.com/watch?v=VIDEO_ID
    Regex("[?&]v=([A-Za-z0-9_-]{11})").find(url)?.groupValues?.get(1)?.let { return it }
    // youtube.com/embed/VIDEO_ID
    Regex("youtube\\.com/embed/([A-Za-z0-9_-]{11})").find(url)?.groupValues?.get(1)?.let { return it }
    return null
}

private fun youtubeThumbnailUrl(url: String): String? {
    val id = youtubeVideoId(url) ?: return null
    return "https://img.youtube.com/vi/$id/hqdefault.jpg"
}

private fun extractLink(intent: Intent?): String? {
    if (intent == null) return null

    return when (intent.action) {
        Intent.ACTION_VIEW -> intent.data?.toString()?.takeIf { isDownloadable(it) }
        Intent.ACTION_SEND -> {
            val text = intent.getStringExtra(Intent.EXTRA_TEXT)
                ?: intent.getStringExtra(Intent.EXTRA_SUBJECT)
                ?: return null
            val match = Regex("https?://\\S+|magnet:\\?\\S+").find(text) ?: return null
            match.value.trimEnd('.', ',', ')', ']', '}', '"', '\'', '>')
        }
        else -> null
    }
}

private fun isDownloadable(value: String): Boolean =
    value.startsWith("http://") || value.startsWith("https://") || value.startsWith("magnet:")

/** Copies a shared .torrent document into app storage so it can be queued. */
private fun copyTorrent(context: Context, uri: Uri): File? = runCatching {
    val target = File(context.cacheDir, "picked-${System.currentTimeMillis()}.torrent")
    context.contentResolver.openInputStream(uri)?.use { input ->
        target.outputStream().use { output -> input.copyTo(output) }
    } ?: return null
    target.takeIf { it.length() > 0L }
}.getOrNull()

private fun fileNameOf(link: String?): String? {
    if (link == null) return null
    val path = link.substringBefore('?').substringBefore('#')
    val name = path.substringAfterLast('/')
    return name.ifBlank { "download" }
}
