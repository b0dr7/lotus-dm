package com.b0dr7.lotusdm

import android.content.Intent
import android.os.Bundle
import android.widget.Toast
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
import androidx.compose.foundation.BorderStroke
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.shape.RoundedCornerShape
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

/**
 * Receives links from browsers (ACTION_VIEW) and the share sheet
 * (ACTION_SEND). Nothing starts until the button here is pressed.
 */
class AddActivity : ComponentActivity() {

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)

        val link = extractLink(intent)
        val mime = intent?.type

        setContent {
            val store = remember { SettingsStore(applicationContext) }
            LotusTheme(mode = store.themeMode) {
                AddScreen(
                    link = link,
                    mime = mime,
                    onConfirm = {
                        if (link != null) {
                            DownloadEngine.get(applicationContext).add(link)
                            DownloadForegroundService.start(applicationContext)
                            Toast.makeText(
                                applicationContext,
                                "Added to the queue",
                                Toast.LENGTH_SHORT,
                            ).show()
                        }
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

    Column(
        modifier = Modifier
            .fillMaxSize()
            .padding(horizontal = 20.dp),
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
                "That share did not contain an http(s) link."
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

private fun extractLink(intent: Intent?): String? {
    if (intent == null) return null

    return when (intent.action) {
        Intent.ACTION_VIEW -> intent.data?.toString()?.takeIf { isHttp(it) }
        Intent.ACTION_SEND -> {
            val text = intent.getStringExtra(Intent.EXTRA_TEXT)
                ?: intent.getStringExtra(Intent.EXTRA_SUBJECT)
                ?: return null
            val match = Regex("https?://\\S+").find(text) ?: return null
            match.value.trimEnd('.', ',', ')', ']', '}', '"', '\'', '>')
        }
        else -> null
    }
}

private fun isHttp(value: String): Boolean =
    value.startsWith("http://") || value.startsWith("https://")

private fun fileNameOf(link: String?): String? {
    if (link == null) return null
    val path = link.substringBefore('?').substringBefore('#')
    val name = path.substringAfterLast('/')
    return name.ifBlank { "download" }
}
