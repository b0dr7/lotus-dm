package com.b0dr7.lotusdm.ui

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.WindowInsets
import androidx.compose.foundation.layout.fillMaxHeight
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.safeDrawing
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.windowInsetsPadding
import androidx.compose.material3.Icon
import androidx.compose.material3.NavigationBar
import androidx.compose.material3.NavigationBarItem
import androidx.compose.material3.NavigationBarItemDefaults
import androidx.compose.material3.NavigationRail
import androidx.compose.material3.NavigationRailItem
import androidx.compose.material3.NavigationRailItemDefaults
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.res.painterResource
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import com.b0dr7.lotusdm.R
import com.b0dr7.lotusdm.ui.screens.HomeScreen
import com.b0dr7.lotusdm.ui.screens.QueueScreen
import com.b0dr7.lotusdm.ui.screens.SettingsScreen
import com.b0dr7.lotusdm.ui.theme.LotusColors
import com.b0dr7.lotusdm.ui.theme.LocalLotusColors
import com.b0dr7.lotusdm.ui.theme.ThemeMode

private data class Tab(val label: String, val icon: Int)

private val tabs = listOf(
    Tab("Home", R.drawable.ic_home),
    Tab("Queue", R.drawable.ic_queue),
    Tab("Settings", R.drawable.ic_settings),
)

@Composable
fun LotusApp(
    mode: ThemeMode,
    onModeChange: (ThemeMode) -> Unit,
    speedLimitKb: Int,
    onSpeedLimitChange: (Int) -> Unit,
) {
    val colors = LocalLotusColors.current
    val window = rememberWindowClass()
    var selected by rememberSaveable { mutableIntStateOf(0) }

    val screen: @Composable () -> Unit = {
        when (selected) {
            0 -> HomeScreen()
            1 -> QueueScreen()
            else -> SettingsScreen(
                mode = mode,
                onModeChange = onModeChange,
                speedLimitKb = speedLimitKb,
                onSpeedLimitChange = onSpeedLimitChange,
            )
        }
    }

    // Tablets, unfolded foldables and landscape get a rail; phones and the
    // cover display of a flip phone keep the bottom bar.
    if (window.usesRail) {
        Row(
            modifier = Modifier
                .fillMaxSize()
                .background(colors.background)
                .windowInsetsPadding(WindowInsets.safeDrawing),
        ) {
            LotusRail(colors = colors, selected = selected, onSelect = { selected = it })
            Box(modifier = Modifier.weight(1f).fillMaxHeight()) { screen() }
        }
    } else {
        Scaffold(
            containerColor = colors.background,
            bottomBar = {
                NavigationBar(
                    containerColor = colors.surface,
                    contentColor = colors.muted,
                    tonalElevation = 0.dp,
                ) {
                    tabs.forEachIndexed { index, tab ->
                        NavigationBarItem(
                            selected = selected == index,
                            onClick = { selected = index },
                            icon = {
                                Icon(
                                    painter = painterResource(tab.icon),
                                    contentDescription = tab.label,
                                    modifier = Modifier.size(22.dp),
                                )
                            },
                            label = { Text(tab.label, fontSize = 12.sp) },
                            colors = NavigationBarItemDefaults.colors(
                                selectedIconColor = colors.accent,
                                selectedTextColor = colors.accent,
                                indicatorColor = colors.accent.copy(alpha = 0.16f),
                                unselectedIconColor = colors.muted,
                                unselectedTextColor = colors.muted,
                            ),
                        )
                    }
                }
            },
        ) { padding ->
            Box(modifier = Modifier.fillMaxSize().padding(padding)) { screen() }
        }
    }
}

@Composable
private fun LotusRail(colors: LotusColors, selected: Int, onSelect: (Int) -> Unit) {
    NavigationRail(
        containerColor = colors.surface,
        contentColor = colors.muted,
    ) {
        tabs.forEachIndexed { index, tab ->
            NavigationRailItem(
                selected = selected == index,
                onClick = { onSelect(index) },
                icon = {
                    Icon(
                        painter = painterResource(tab.icon),
                        contentDescription = tab.label,
                        modifier = Modifier.size(22.dp),
                    )
                },
                label = { Text(tab.label, fontSize = 12.sp) },
                alwaysShowLabel = true,
                colors = NavigationRailItemDefaults.colors(
                    selectedIconColor = colors.accent,
                    selectedTextColor = colors.accent,
                    indicatorColor = colors.accent.copy(alpha = 0.16f),
                    unselectedIconColor = colors.muted,
                    unselectedTextColor = colors.muted,
                ),
            )
        }
    }
}
