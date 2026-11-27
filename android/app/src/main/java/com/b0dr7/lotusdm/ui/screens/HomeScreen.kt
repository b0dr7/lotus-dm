package com.b0dr7.lotusdm.ui.screens

import androidx.compose.foundation.BorderStroke
import androidx.compose.foundation.Image
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
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.res.painterResource
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import com.b0dr7.lotusdm.R
import com.b0dr7.lotusdm.ui.theme.LocalLotusColors

@Composable
fun HomeScreen() {
    val colors = LocalLotusColors.current

    Column(
        modifier = Modifier
            .fillMaxSize()
            .verticalScroll(rememberScrollState())
            .padding(horizontal = 20.dp),
    ) {
        Spacer(Modifier.height(28.dp))

        Row(verticalAlignment = Alignment.CenterVertically) {
            Box(
                modifier = Modifier
                    .size(46.dp)
                    .clip(RoundedCornerShape(12.dp))
                    .background(colors.surfaceAlt),
                contentAlignment = Alignment.Center,
            ) {
                Image(
                    painter = painterResource(R.drawable.ic_launcher_foreground),
                    contentDescription = null,
                    modifier = Modifier.size(42.dp),
                )
            }
            Spacer(Modifier.size(12.dp))
            Column {
                Text(
                    text = "Lotus DM",
                    color = colors.text,
                    fontSize = 22.sp,
                    fontWeight = FontWeight.SemiBold,
                )
                Text(
                    text = "Download manager",
                    color = colors.muted,
                    fontSize = 13.sp,
                )
            }
        }

        Spacer(Modifier.height(28.dp))

        Surface(
            modifier = Modifier.fillMaxWidth(),
            shape = RoundedCornerShape(16.dp),
            color = colors.surface,
            border = BorderStroke(1.dp, colors.border),
        ) {
            Column(modifier = Modifier.padding(18.dp)) {
                Text(
                    text = "Queue",
                    color = colors.muted,
                    fontSize = 12.sp,
                    fontWeight = FontWeight.Medium,
                )
                Spacer(Modifier.height(6.dp))
                Text(
                    text = "0 active downloads",
                    color = colors.text,
                    fontSize = 24.sp,
                    fontWeight = FontWeight.SemiBold,
                )
                Spacer(Modifier.height(14.dp))
                Row(
                    horizontalArrangement = Arrangement.spacedBy(8.dp),
                    verticalAlignment = Alignment.CenterVertically,
                ) {
                    Badge(text = "Idle")
                    Badge(text = "8 segments")
                }
            }
        }

        Spacer(Modifier.height(16.dp))

        Text(
            text = "Downloads you start here will show up in the queue.",
            color = colors.muted,
            fontSize = 13.sp,
        )

        Spacer(Modifier.height(28.dp))
    }
}

@Composable
private fun Badge(text: String) {
    val colors = LocalLotusColors.current
    Surface(
        shape = RoundedCornerShape(999.dp),
        color = colors.accent.copy(alpha = 0.14f),
        contentColor = colors.accent,
    ) {
        Text(
            text = text,
            fontSize = 12.sp,
            fontWeight = FontWeight.Medium,
            modifier = Modifier.padding(horizontal = 10.dp, vertical = 4.dp),
        )
    }
}
