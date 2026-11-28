package com.b0dr7.lotusdm.ui.screens

import androidx.compose.foundation.BorderStroke
import androidx.compose.foundation.horizontalScroll
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.FilterChip
import androidx.compose.material3.FilterChipDefaults
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Surface
import androidx.compose.material3.Switch
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import com.b0dr7.lotusdm.data.SettingsStore
import com.b0dr7.lotusdm.data.TrafficMode
import com.b0dr7.lotusdm.ui.Page
import com.b0dr7.lotusdm.ui.rememberWindowClass
import com.b0dr7.lotusdm.ui.theme.LocalLotusColors
import com.b0dr7.lotusdm.ui.theme.ThemeMode

@Composable
fun SettingsScreen(
    mode: ThemeMode,
    onModeChange: (ThemeMode) -> Unit,
    store: SettingsStore,
    onSettingsChanged: () -> Unit,
) {
    val colors = LocalLotusColors.current
    val window = rememberWindowClass()

    var trafficMode by remember { mutableStateOf(store.trafficMode) }
    var customLimit by remember { mutableIntStateOf(store.speedLimitKb) }
    var wifiOnly by remember { mutableStateOf(store.wifiOnly) }
    var notifyResults by remember { mutableStateOf(store.notifyResults) }
    var concurrent by remember { mutableIntStateOf(store.maxConcurrent) }
    var launchAtStartup by remember { mutableStateOf(store.launchAtStartup) }

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
        SectionLabel("Traffic mode")

        Row(
            modifier = Modifier.horizontalScroll(rememberScrollState()),
            horizontalArrangement = Arrangement.spacedBy(8.dp),
        ) {
            TrafficMode.entries.forEach { candidate ->
                FilterChip(
                    selected = trafficMode == candidate,
                    onClick = {
                        trafficMode = candidate
                        store.trafficMode = candidate
                        onSettingsChanged()
                    },
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

        Spacer(Modifier.height(10.dp))
        Text(
            text = if (trafficMode.isCustom) {
                "Custom uses the limit below for every download."
            } else {
                "Fixed at ${trafficMode.kbps} KB/s (0 means no limit). A single row can still override it."
            },
            color = colors.muted,
            fontSize = 13.sp,
        )

        if (trafficMode.isCustom) {
            Spacer(Modifier.height(10.dp))
            OutlinedTextField(
                value = customLimit.toString(),
                onValueChange = { raw ->
                    val digits = raw.filter { it.isDigit() }.take(6)
                    customLimit = digits.toIntOrNull() ?: 0
                    store.speedLimitKb = customLimit
                    onSettingsChanged()
                },
                modifier = Modifier.fillMaxWidth(),
                singleLine = true,
                label = { Text("Speed limit in KB/s", fontSize = 13.sp) },
                shape = RoundedCornerShape(12.dp),
                keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Number),
            )
        }

        Spacer(Modifier.height(24.dp))
        SectionLabel("Connection")

        ToggleRow(
            title = "Wi-Fi only",
            detail = "Hold the queue on mobile data and pick it up again on Wi-Fi.",
            checked = wifiOnly,
            onCheckedChange = {
                wifiOnly = it
                store.wifiOnly = it
                onSettingsChanged()
            },
        )
        Spacer(Modifier.height(8.dp))
        ToggleRow(
            title = "Notify when a download ends",
            detail = "One alert when a download finishes, or when it finally fails.",
            checked = notifyResults,
            onCheckedChange = {
                notifyResults = it
                store.notifyResults = it
                onSettingsChanged()
            },
        )
        Spacer(Modifier.height(8.dp))
        ToggleRow(
            title = "Launch at startup",
            detail = "Pick the saved queue back up after the phone reboots.",
            checked = launchAtStartup,
            onCheckedChange = {
                launchAtStartup = it
                store.launchAtStartup = it
                onSettingsChanged()
            },
        )

        Spacer(Modifier.height(16.dp))
        Text(text = "Downloads at once", color = colors.text, fontSize = 15.sp)
        Spacer(Modifier.height(6.dp))
        Row(
            modifier = Modifier.horizontalScroll(rememberScrollState()),
            horizontalArrangement = Arrangement.spacedBy(8.dp),
        ) {
            (1..6).forEach { count ->
                FilterChip(
                    selected = concurrent == count,
                    onClick = {
                        concurrent = count
                        store.maxConcurrent = count
                        onSettingsChanged()
                    },
                    label = { Text(count.toString(), fontSize = 12.sp) },
                    shape = RoundedCornerShape(999.dp),
                    colors = FilterChipDefaults.filterChipColors(
                        selectedContainerColor = colors.accent.copy(alpha = 0.16f),
                        selectedLabelColor = colors.accent,
                        labelColor = colors.muted,
                    ),
                )
            }
        }
        Spacer(Modifier.height(6.dp))
        Text(
            text = "Extra downloads wait in the queue; high priority rows go first.",
            color = colors.muted,
            fontSize = 13.sp,
        )

        Spacer(Modifier.height(24.dp))
        Text(
            text = "Themes, limits and queue rules are remembered on this device.",
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
private fun ToggleRow(
    title: String,
    detail: String,
    checked: Boolean,
    onCheckedChange: (Boolean) -> Unit,
) {
    val colors = LocalLotusColors.current
    Surface(
        modifier = Modifier.fillMaxWidth(),
        shape = RoundedCornerShape(12.dp),
        color = colors.surface,
        border = BorderStroke(1.dp, colors.border),
    ) {
        Row(
            modifier = Modifier.padding(horizontal = 16.dp, vertical = 12.dp),
            verticalAlignment = Alignment.CenterVertically,
        ) {
            Column(modifier = Modifier.weight(1f)) {
                Text(text = title, color = colors.text, fontSize = 15.sp)
                Spacer(Modifier.height(2.dp))
                Text(text = detail, color = colors.muted, fontSize = 12.sp)
            }
            Spacer(Modifier.width(12.dp))
            Switch(checked = checked, onCheckedChange = onCheckedChange)
        }
    }
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
