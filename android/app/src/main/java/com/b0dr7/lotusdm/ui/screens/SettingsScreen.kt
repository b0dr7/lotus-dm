package com.b0dr7.lotusdm.ui.screens

import androidx.compose.foundation.BorderStroke
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import com.b0dr7.lotusdm.ui.Page
import com.b0dr7.lotusdm.ui.rememberWindowClass
import com.b0dr7.lotusdm.ui.theme.LocalLotusColors
import com.b0dr7.lotusdm.ui.theme.ThemeMode

@Composable
fun SettingsScreen(
    mode: ThemeMode,
    onModeChange: (ThemeMode) -> Unit,
    speedLimitKb: Int,
    onSpeedLimitChange: (Int) -> Unit,
) {
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
            text = "Settings",
            color = colors.text,
            fontSize = 26.sp,
            fontWeight = FontWeight.SemiBold,
        )

        Spacer(Modifier.height(24.dp))
        SectionLabel("Appearance")

        ThemeMode.values().forEach { candidate ->
            ThemeRow(
                mode = candidate,
                selected = candidate == mode,
                onClick = { onModeChange(candidate) },
            )
            Spacer(Modifier.height(8.dp))
        }

        Spacer(Modifier.height(24.dp))
        SectionLabel("Bandwidth")

        var limitText by remember { mutableStateOf(speedLimitKb.toString()) }

        OutlinedTextField(
            value = limitText,
            onValueChange = { raw ->
                val digits = raw.filter { it.isDigit() }.take(6)
                limitText = digits
                onSpeedLimitChange(digits.toIntOrNull() ?: 0)
            },
            modifier = Modifier.fillMaxWidth(),
            singleLine = true,
            label = { Text("Speed limit in KB/s", fontSize = 13.sp) },
            shape = RoundedCornerShape(12.dp),
            keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Number),
        )

        Spacer(Modifier.height(8.dp))
        Text(
            text = "0 means no limit. This applies to every download; a single row can override it.",
            color = colors.muted,
            fontSize = 13.sp,
        )

        Spacer(Modifier.height(24.dp))
        Text(
            text = "Themes and limits are remembered on this device.",
            color = colors.muted,
            fontSize = 13.sp,
        )
        Spacer(Modifier.height(28.dp))
    }
    }
}

@Composable
private fun SectionLabel(text: String) {
    val colors = LocalLotusColors.current
    Text(
        text = text,
        color = colors.muted,
        fontSize = 12.sp,
        fontWeight = FontWeight.Medium,
    )
    Spacer(Modifier.height(10.dp))
}

@Composable
private fun ThemeRow(mode: ThemeMode, selected: Boolean, onClick: () -> Unit) {
    val colors = LocalLotusColors.current

    Surface(
        onClick = onClick,
        modifier = Modifier.fillMaxWidth(),
        shape = RoundedCornerShape(12.dp),
        color = if (selected) colors.accent.copy(alpha = 0.12f) else colors.surface,
        border = BorderStroke(1.dp, if (selected) colors.accent else colors.border),
    ) {
        Row(
            modifier = Modifier.padding(horizontal = 16.dp, vertical = 14.dp),
            verticalAlignment = Alignment.CenterVertically,
        ) {
            Text(
                text = mode.label,
                color = if (selected) colors.accent else colors.text,
                fontSize = 15.sp,
                fontWeight = if (selected) FontWeight.SemiBold else FontWeight.Normal,
            )
            Spacer(Modifier.weight(1f))
            if (selected) {
                Text(
                    text = "Active",
                    color = colors.accent,
                    fontSize = 12.sp,
                    fontWeight = FontWeight.Medium,
                )
            }
        }
    }
}
