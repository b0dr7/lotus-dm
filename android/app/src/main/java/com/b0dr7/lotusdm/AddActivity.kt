package com.b0dr7.lotusdm

import android.content.Context
import android.content.Intent
import android.net.Uri
import android.os.Bundle
import android.widget.Toast
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
import androidx.compose.foundation.BorderStroke
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.imePadding
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.Button
import androidx.compose.material3.ButtonDefaults
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.remember
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import com.b0dr7.lotusdm.data.SettingsStore
import com.b0dr7.lotusdm.engine.DownloadEngine
import com.b0dr7.lotusdm.service.DownloadForegroundService
import com.b0dr7.lotusdm.ui.theme.LocalLotusColors
import com.b0dr7.lotusdm.ui.theme.LotusTheme
import java.io.File

/**
 * Receives links from browsers (ACTION_VIEW) and the share sheet
 * (ACTION_SEND). Nothing starts until the button here is pressed.
 */
private const val MIME_TORRENT = "application/x-bittorrent"

class AddActivity : ComponentActivity() {

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)

        val link = extractLink(intent)
        val mime = intent?.type
        val torrent = intent?.data?.takeIf { mime == MIME_TORRENT }

        setContent {
            val store = remember { SettingsStore(applicationContext) }
            LotusTheme(mode = store.themeMode) {
                AddScreen(
                    link = link ?: torrent?.toString(),
                    mime = mime,
                    onConfirm = {
                        val queued = if (torrent != null) {
                            val copy = copyTorrent(applicationContext, torrent)
                            copy?.let { DownloadEngine.get(applicationContext).addTorrentFile(it) }
                        } else {
                            link?.let { DownloadEngine.get(applicationContext).add(it) }
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

@Composable
private fun AddScreen(
    link: String?,
    mime: String?,
    onConfirm: () -> Unit,
    onCancel: () -> Unit,
) {
    val colors = LocalLotusColors.current
    val valid = link != null

    // Cover displays of flip phones are short and narrow: scroll instead of
    // clipping, and keep the card from stretching on a tablet.
    Box(
        modifier = Modifier
            .fillMaxSize()
            .imePadding(),
        contentAlignment = Alignment.Center,
    ) {
    Column(
        modifier = Modifier
            .widthIn(max = 560.dp)
            .fillMaxWidth()
            .verticalScroll(rememberScrollState())
            .padding(horizontal = 20.dp, vertical = 24.dp),
        verticalArrangement = Arrangement.Center,
    ) {
        Text(
            text = if (valid) "Add to Lotus DM" else "Nothing to download",
            color = colors.text,
            fontSize = 24.sp,
            fontWeight = FontWeight.SemiBold,
        )
        Spacer(Modifier.height(10.dp))
        Text(
            text = if (valid) {
                "This link was handed to Lotus DM by another app."
            } else {
                "That share did not contain a link or a .torrent file."
            },
            color = colors.muted,
            fontSize = 13.sp,
        )

        Spacer(Modifier.height(18.dp))

        Surface(
            modifier = Modifier.fillMaxWidth(),
            shape = RoundedCornerShape(14.dp),
            color = colors.surface,
            border = BorderStroke(1.dp, colors.border),
        ) {
            Column(modifier = Modifier.padding(16.dp)) {
                Text(
                    text = fileNameOf(link) ?: "-",
                    color = colors.text,
                    fontSize = 15.sp,
                    fontWeight = FontWeight.Medium,
                    maxLines = 2,
                    overflow = TextOverflow.Ellipsis,
                )
                Spacer(Modifier.height(6.dp))
                Text(
                    text = link ?: "-",
                    color = colors.muted,
                    fontSize = 12.sp,
                    maxLines = 3,
                    overflow = TextOverflow.Ellipsis,
                )
                if (mime != null) {
                    Spacer(Modifier.height(8.dp))
                    Text(
                        text = mime,
                        color = colors.muted,
                        fontSize = 11.sp,
                        maxLines = 1,
                        overflow = TextOverflow.Ellipsis,
                    )
                }
            }
        }

        Spacer(Modifier.height(20.dp))

        Button(
            onClick = onConfirm,
            enabled = valid,
            modifier = Modifier.fillMaxWidth(),
            shape = RoundedCornerShape(12.dp),
            colors = ButtonDefaults.buttonColors(
                containerColor = colors.accent,
                contentColor = colors.onAccent,
            ),
        ) {
            Text("Start download", fontWeight = FontWeight.SemiBold)
        }

        Spacer(Modifier.height(6.dp))

        TextButton(
            onClick = onCancel,
            modifier = Modifier.fillMaxWidth(),
        ) {
            Text("Cancel", color = colors.muted)
        }

        Spacer(Modifier.height(12.dp))
        Text(
            text = "Nothing is queued until you press Start download.",
            color = colors.muted,
            fontSize = 12.sp,
        )
    }
    }
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
