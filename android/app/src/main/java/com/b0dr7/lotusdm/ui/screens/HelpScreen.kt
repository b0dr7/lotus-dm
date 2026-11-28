package com.b0dr7.lotusdm.ui.screens

import androidx.compose.foundation.BorderStroke
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ColumnScope
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import com.b0dr7.lotusdm.ui.Page
import com.b0dr7.lotusdm.ui.rememberWindowClass
import com.b0dr7.lotusdm.ui.theme.LocalLotusColors

@Composable
fun HelpScreen() {
    val colors = LocalLotusColors.current
    val window = rememberWindowClass()

    Page(window) { pageModifier ->
        Column(
            modifier = pageModifier
                .verticalScroll(rememberScrollState())
                .padding(horizontal = window.gutter),
        ) {
            Spacer(Modifier.height(if (window.isMultiColumn) 36.dp else 28.dp))
            Text(
                text = "Help",
                color = colors.text,
                fontSize = 26.sp,
                fontWeight = FontWeight.SemiBold,
            )
            Spacer(Modifier.height(6.dp))
            Text(
                text = "Sending links, the queue, and what to check when a download does not start.",
                color = colors.muted,
                fontSize = 13.sp,
            )
            Spacer(Modifier.height(24.dp))

            HelpCard("Send a link to Lotus DM") {
                HelpLine(
                    "In a browser or any app with a Share button: tap Share, pick Lotus DM, and the link opens the add sheet.",
                )
                HelpLine(
                    "Long-press a link and choose Open with, then Lotus DM. Magnet links and .torrent files open the same sheet.",
                )
            }

            HelpCard("The add sheet") {
                HelpLine(
                    "It shows the link, a name you can change, an optional delay, and mirrors. Nothing downloads until you press Download.",
                )
                HelpLine(
                    "Downloads keep running when you leave the app, and a notification shows progress while they do.",
                )
            }

            HelpCard("The queue") {
                HelpLine(
                    "Every job can be paused, resumed, cancelled or renamed from the queue.",
                )
                HelpLine(
                    "Renaming a finished download renames the file on disk, not just the label in the list.",
                )
            }

            HelpCard("If a download does not start") {
                HelpLine(
                    "With Wi-Fi only turned on, a job waits for Wi-Fi instead of using mobile data.",
                )
                HelpLine(
                    "If you denied the notification permission, Android may stop the download service. Allow notifications for Lotus DM in the system settings.",
                )
                HelpLine(
                    "Battery optimisation can kill long downloads. Exempt Lotus DM from it if a job keeps stopping.",
                )
            }

            HelpCard("There is no browser extension on Android") {
                HelpLine(
                    "The extension talks to the Lotus app on the same PC over 127.0.0.1, so it exists for Windows only. On Android the Share sheet does the same job.",
                )
            }

            HelpCard("Privacy") {
                HelpLine(
                    "No account and no server in the middle: Lotus talks to the site you download from and to nothing else.",
                )
            }

            Spacer(Modifier.height(28.dp))
        }
    }
}

@Composable
private fun HelpCard(title: String, body: @Composable ColumnScope.() -> Unit) {
    val colors = LocalLotusColors.current
    Surface(
        modifier = Modifier.fillMaxWidth(),
        shape = RoundedCornerShape(16.dp),
        color = colors.surface,
        border = BorderStroke(1.dp, colors.border),
    ) {
        Column(modifier = Modifier.padding(18.dp)) {
            Text(
                text = title,
                color = colors.text,
                fontSize = 15.sp,
                fontWeight = FontWeight.SemiBold,
            )
            Spacer(Modifier.height(8.dp))
            body()
        }
    }
    Spacer(Modifier.height(14.dp))
}

@Composable
private fun HelpLine(text: String) {
    val colors = LocalLotusColors.current
    Text(
        text = text,
        color = colors.muted,
        fontSize = 13.sp,
        lineHeight = 19.sp,
    )
    Spacer(Modifier.height(8.dp))
}
