use eframe::egui::{self, RichText};

use crate::ui::theme::Palette;

pub fn help_panel(ui: &mut egui::Ui, p: &Palette) {
    ui.add_space(16.0);
    ui.heading("Help");
    ui.label(
        RichText::new("Browser extension and troubleshooting.")
            .color(p.muted)
            .size(12.5),
    );
    ui.add_space(16.0);

    ui.heading("What the extension does");
    ui.label(
        "Right-click a link and choose \"Download with Lotus DM\", or open the toolbar popup to send the page you are on. The extension hands the link to the running app over 127.0.0.1 and the download joins the same queue as everything else. Nothing leaves your machine - there is no account and no server in the middle.",
    );
    ui.label(RichText::new("Windows only. The extension talks to the Lotus app on the same PC, so that app has to be open to accept a link. Android has no extension - share a link to Lotus DM from any browser instead.").color(p.muted).size(12.0));
    ui.add_space(16.0);

    ui.heading("Firefox");
    ui.label("One click from addons.mozilla.org installs it into Firefox like any other add-on, updates included.");
    ui.label(RichText::new("The listing is in review. Until it is approved, use the Chromium steps below on Windows.").color(p.muted).size(12.0));
    ui.add_space(16.0);

    ui.heading("Chrome, Edge, Brave, Opera");
    ui.label("Chromium browsers need the folder once, then they keep it. It takes about a minute.");
    ui.label("1. Unzip the extension into a folder you will keep, for example Documents\\Lotus-extension.");
    ui.label("2. Open chrome://extensions - in Edge that is edge://extensions.");
    ui.label("4. Click Load unpacked and pick the folder you unzipped.");
    ui.label("5. Pin Lotus to the toolbar from the puzzle-piece menu, so the icon is one click away.");
    ui.label("6. Start Lotus Download Manager, then click the Lotus icon: the badge should read connected.");
    ui.label(RichText::new("Keep that folder where it is. If you move or delete it, the extension stops working and has to be loaded again. To update it later, replace the files inside the folder and press the reload arrow on the extension card.").color(p.muted).size(12.0));
    ui.add_space(16.0);

    ui.heading("If the badge says \"app not running\"");
    ui.label("With Lotus Download Manager open, visit http://127.0.0.1:8460/ping in the same browser. You should see {\"ok\":true,...}.");
    ui.label("- Something else answers, or the port is busy: change the port in that same settings block, restart the app, then type the new port into the extension popup and press Save.");
    ui.add_space(16.0);

    ui.label(RichText::new("The extension asks for one host permission, http://127.0.0.1/*, and nothing else. It reads the tab you send and the link you right-click. No analytics, no remote code.").color(p.muted).size(12.0));