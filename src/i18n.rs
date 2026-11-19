//! Bundled Fluent messages and the application's explicit language boundary.
use enzui_core::localization::{
    Arguments, Catalog, CatalogBuilder, Locale, Localizer, preferred_locales,
};
use std::sync::OnceLock;

/// The installer's languages: recovery speaks whatever the system was installed in.
const BUNDLED: [(&str, &str); 10] = [
    ("en-US", include_str!("../assets/locales/en-US.ftl")),
    ("de", include_str!("../assets/locales/de.ftl")),
    ("es", include_str!("../assets/locales/es.ftl")),
    ("fr", include_str!("../assets/locales/fr.ftl")),
    ("it", include_str!("../assets/locales/it.ftl")),
    ("hu", include_str!("../assets/locales/hu.ftl")),
    ("nl", include_str!("../assets/locales/nl.ftl")),
    ("pl", include_str!("../assets/locales/pl.ftl")),
    ("pt", include_str!("../assets/locales/pt.ftl")),
    ("sv", include_str!("../assets/locales/sv.ftl")),
];

fn catalog() -> Catalog {
    static CATALOG: OnceLock<Catalog> = OnceLock::new();
    CATALOG
        .get_or_init(|| {
            let mut builder = CatalogBuilder::new();
            for (locale, resource) in BUNDLED {
                builder
                    .add_resource(locale, resource)
                    .expect("valid bundled translations");
            }
            builder.build()
        })
        .clone()
}

#[cfg(test)]
pub(crate) fn for_locale(locale: &str) -> Localizer {
    let preferred = Locale::from_posix(locale).ok().into_iter().collect();
    Localizer::new(catalog(), preferred, Locale::default())
}

pub(crate) fn localizer() -> &'static Localizer {
    static LOCALIZER: OnceLock<Localizer> = OnceLock::new();
    LOCALIZER.get_or_init(|| Localizer::new(catalog(), preferred_locales(), Locale::default()))
}

pub(crate) fn text(source: &str) -> String {
    translate(localizer(), source)
}

pub(crate) fn translate(localizer: &Localizer, source: &str) -> String {
    match source_key(source) {
        Some(key) => localizer.text(key),
        None => source.to_owned(),
    }
}

pub(crate) fn format(
    localizer: &Localizer,
    source: &str,
    values: &[(&str, &dyn enzui_core::localization::MessageArgument)],
) -> String {
    let Some(key) = source_key(source) else {
        return source.to_owned();
    };
    let mut args = Arguments::new();
    for (name, value) in values {
        args.set(*name, value.message_value());
    }
    localizer.text_with(key, &args)
}

pub(crate) fn source_key(source: &str) -> Option<&'static str> {
    MESSAGES
        .iter()
        .find_map(|(en, key)| (*en == source).then_some(*key))
}

pub(crate) fn labels(localizer: &Localizer) -> std::collections::HashMap<&'static str, String> {
    MESSAGES
        .iter()
        .filter(|(source, _)| !source.contains('{'))
        .map(|(source, key)| (*source, localizer.text(key)))
        .collect()
}

// Static presentation getters share one bounded set of startup translations.
pub(crate) fn label(source: &'static str) -> &'static str {
    static LABELS: OnceLock<std::collections::HashMap<&'static str, String>> = OnceLock::new();
    LABELS
        .get_or_init(|| labels(localizer()))
        .get(source)
        .map_or(source, String::as_str)
}

pub(crate) fn decimal(digits: &str) -> String {
    digits
        .parse::<enzui_core::localization::Decimal>()
        .ok()
        .and_then(|value| {
            enzui_core::localization::current()
                .locale()
                .format_decimal(&value)
                .ok()
        })
        .unwrap_or_else(|| digits.to_owned())
}

#[rustfmt::skip]
const MESSAGES: &[(&str, &str)] = &[
    (" · {} unsupported items skipped", "app-unsupported-items-skipped-1ed06b"),
    ("A full-screen console for command-line recovery work", "app-a-full-screen-console-for-command-line-recovery-work-10b0c9"),
    ("A new system root will be created on the next boot. Your home folder, personal files and package trust are kept. The current system is retained.", "app-a-new-system-root-will-be-created-on-the-next-boot-your-home-fol-6f8c25"),
    ("Administrator", "app-administrator-e7d3e7"),
    ("Administrator password", "app-administrator-password-f93c60"),
    ("Back", "app-back-76900f"),
    ("Back up files", "app-back-up-files-c2c057"),
    ("Back up your files", "app-back-up-your-files-6a2733"),
    ("Backup drive", "app-backup-drive-e06956"),
    ("Backup password", "app-backup-password-3e3970"),
    ("Backup server (REST)", "app-backup-server-rest-e43a41"),
    ("Boot diagnostics", "app-boot-diagnostics-0d4dbb"),
    ("Boot log QR code", "app-boot-log-qr-code-94a48a"),
    ("Boot logs", "app-boot-logs-5a8cf0"),
    ("Bring back your files from an encrypted backup", "app-bring-back-your-files-from-an-encrypted-backup-bb8992"),
    ("Bring back your files from an encrypted backup on a drive, a network share or a server.", "app-bring-back-your-files-from-an-encrypted-backup-on-a-drive-a-netw-bd72d5"),
    ("Browsing action unavailable", "app-browsing-action-unavailable-5a8f5b"),
    ("Browsing authentication failed", "app-browsing-authentication-failed-22a858"),
    ("Browsing authentication timed out.", "app-browsing-authentication-timed-out-87325f"),
    ("Browsing connection closed", "app-browsing-connection-closed-86bce5"),
    ("Browsing response did not match the request.", "app-browsing-response-did-not-match-the-request-80b443"),
    ("Cancel", "app-cancel-19766e"),
    ("Cancel copy", "app-cancel-copy-3ea61e"),
    ("Choose a restore point", "app-choose-a-restore-point-0ac333"),
    ("Choose a snapshot", "app-choose-a-snapshot-b936f7"),
    ("Choose what you need to do", "app-choose-what-you-need-to-do-1a80b0"),
    ("Close", "app-close-7d9eb7"),
    ("Confirm recovered system", "app-confirm-recovered-system-3f6181"),
    ("Confirm the restored system so normal updates can resume.", "app-confirm-the-restored-system-so-normal-updates-can-resume-0c3832"),
    ("Connect an external drive", "app-connect-an-external-drive-307ca1"),
    ("Copied and verified {count} items", "app-copy-result-0592b5"),
    ("Copied and verified {count} items; {skipped} skipped", "app-copy-result-8a8e32"),
    ("Copied and verified {}", "app-copied-and-verified-1d8ac2"),
    ("Copy", "app-copy-e21f93"),
    ("Copy important files to a USB drive or external hard drive.", "app-copy-important-files-to-a-usb-drive-or-external-hard-drive-aac21d"),
    ("Copy selected items", "app-copy-selected-items-0d4dda"),
    ("Copy with QR", "app-copy-with-qr-c52036"),
    ("Could not complete the operation", "app-could-not-complete-the-operation-92cebc"),
    ("Could not restore the system", "app-could-not-restore-the-system-131c57"),
    ("Create folder", "app-create-folder-82b9e1"),
    ("Current boot · refreshes automatically", "app-current-boot-refreshes-automatically-c5c988"),
    ("Date unavailable", "app-date-unavailable-f41419"),
    ("Delete", "app-delete-e2d0a5"),
    ("Delete selected items?", "app-delete-selected-items-a9e40c"),
    ("Delete…", "app-delete-9ce78f"),
    ("Drag files from your system to the external drive", "app-drag-files-from-your-system-to-the-external-drive-c64243"),
    ("Eject drive", "app-eject-drive-f19955"),
    ("External drive", "app-external-drive-73a776"),
    ("File operation", "app-file-operation-76b08e"),
    ("Files", "app-files-abc7e9"),
    ("Find backups", "app-find-backups-e9b6a2"),
    ("Find drives", "app-find-drives-12d17c"),
    ("Finding restore points…", "app-finding-restore-points-c22f16"),
    ("Finish recovery", "app-finish-recovery-2031da"),
    ("Folder actions", "app-folder-actions-b7ea53"),
    ("Folder created", "app-folder-created-b1dfe0"),
    ("Folder on the drive", "app-folder-on-the-drive-0c00e0"),
    ("If you switched away from the console, press Ctrl+Alt+F8 to return to it.", "app-if-you-switched-away-from-the-console-press-ctrl-alt-f8-to-retur-a44343"),
    ("Invalid browsing result.", "app-invalid-browsing-result-9330d1"),
    ("Item renamed", "app-item-renamed-288af2"),
    ("Items deleted", "app-items-deleted-ba7511"),
    ("Keep the computer connected to power", "app-keep-the-computer-connected-to-power-31b337"),
    ("Latest boot output", "app-latest-boot-output-3ae1b5"),
    ("Name", "app-name-dcd1d5"),
    ("Network share (SMB)", "app-network-share-smb-16806e"),
    ("New folder", "app-new-folder-cf28f4"),
    ("New folder…", "app-new-folder-6cca52"),
    ("No administrator account is available. Use command-line recovery to inspect the account configuration.", "app-no-administrator-account-is-available-use-command-line-recovery--6f1af0"),
    ("No bootable system snapshots are available yet.", "app-no-bootable-system-snapshots-are-available-yet-95188f"),
    ("No output yet.", "app-no-output-yet-d2e93d"),
    ("No restore points yet", "app-no-restore-points-yet-902f68"),
    ("No space left on device", "app-no-space-left-on-device-2ec76a"),
    ("Open folder", "app-open-folder-6a9084"),
    ("Open terminal", "app-open-terminal-acb1f4"),
    ("Operation completed.", "app-operation-completed-7f52b3"),
    ("Output from the current boot", "app-output-from-the-current-boot-efec2f"),
    ("Package comparison only · personal files are kept", "app-package-comparison-only-personal-files-are-kept-bad3d4"),
    ("Parent folder", "app-parent-folder-158f5a"),
    ("Paste", "app-paste-f3380f"),
    ("Paste here", "app-paste-here-f86b91"),
    ("Permission denied", "app-permission-denied-59b77d"),
    ("Personal backup", "app-personal-backup-22ba9b"),
    ("Power options", "app-power-options-e40690"),
    ("Preview complete. No system changes were made.", "app-preview-complete-no-system-changes-were-made-b29dc2"),
    ("Preview — no system changes", "app-preview-no-system-changes-322c58"),
    ("Read warnings to help find what went wrong.", "app-read-warnings-to-help-find-what-went-wrong-a9a2a8"),
    ("Reading system changes…", "app-reading-system-changes-621cbf"),
    ("Recovery asks for the disk passphrase after restarting. Do not interrupt the restore.", "app-recovery-asks-for-the-disk-passphrase-after-restarting-do-not-in-ee1971"),
    ("Recovery completed", "app-recovery-completed-441bd0"),
    ("Recovery confirmation required", "app-recovery-confirmation-required-e99ea2"),
    ("Recovery interface closed", "app-recovery-interface-closed-bcc675"),
    ("Recovery metadata needs attention", "app-recovery-metadata-needs-attention-8fbe81"),
    ("Recovery operation cancelled.", "app-recovery-operation-cancelled-5e7389"),
    ("Recovery operation wait limit reached. Inspect its result before trying again.", "app-recovery-operation-wait-limit-reached-inspect-its-result-before--de480a"),
    ("Recovery service returned no result.", "app-recovery-service-returned-no-result-fe750a"),
    ("Recovery terminal", "app-recovery-terminal-f7e8f7"),
    ("Refresh", "app-refresh-0e9161"),
    ("Rename", "app-rename-3064d7"),
    ("Rename…", "app-rename-6fa62b"),
    ("Restart computer", "app-restart-computer-f6c152"),
    ("Restart or shut down this computer", "app-restart-or-shut-down-this-computer-47d69a"),
    ("Restore", "app-restore-a76e13"),
    ("Restore a snapshot", "app-restore-a-snapshot-a4acf9"),
    ("Restore a system snapshot", "app-restore-a-system-snapshot-3c0743"),
    ("Restore and restart", "app-restore-and-restart-508fae"),
    ("Restore from a backup", "app-restore-from-a-backup-3e44e6"),
    ("Restore personal files to", "app-restore-personal-files-to-8cc2a2"),
    ("Restore points", "app-restore-points-29f90b"),
    ("Restore system", "app-restore-system-97fa59"),
    ("Restore system files while keeping your personal files.", "app-restore-system-files-while-keeping-your-personal-files-87fe90"),
    ("Restoring system", "app-restoring-system-0abaf0"),
    ("Return system files to an earlier working state", "app-return-system-files-to-an-earlier-working-state-d3d419"),
    ("Return to an earlier working state", "app-return-to-an-earlier-working-state-f9a6c6"),
    ("Return to the boot menu.", "app-return-to-the-boot-menu-86d7c6"),
    ("Review snapshot", "app-review-snapshot-6f0269"),
    ("Save failed", "app-save-failed-53ad6f"),
    ("Saved state could not be opened", "app-saved-state-could-not-be-opened-e6ee25"),
    ("Saved state could not be understood", "app-saved-state-could-not-be-understood-a16a20"),
    ("Saving is blocked to protect existing saved data", "app-saving-is-blocked-to-protect-existing-saved-data-b47fe4"),
    ("Scan to copy · updates every second", "app-scan-to-copy-updates-every-second-1d22d7"),
    ("See what will change before restoring. Your current system is retained so recovery can be reviewed if something goes wrong.", "app-see-what-will-change-before-restoring-your-current-system-is-ret-890de8"),
    ("Select all", "app-select-all-1fc9a3"),
    ("Select an external drive", "app-select-an-external-drive-fc6280"),
    ("Selected restore point", "app-selected-restore-point-6a14f9"),
    ("Server address", "app-server-address-8be897"),
    ("Share", "app-share-29887a"),
    ("Share password", "app-share-password-eeb062"),
    ("Server password", "app-server-password-146e20"),
    ("Show all package changes", "app-show-all-package-changes-128fcf"),
    ("Show less", "app-show-less-94ea9b"),
    ("Shut down", "app-shut-down-a41934"),
    ("Size", "app-size-1af851"),
    ("Snapshot recovery did not finish", "app-snapshot-recovery-did-not-finish-24ff9a"),
    ("Starting recovery…", "app-starting-recovery-ed210f"),
    ("System", "app-system-6725e7"),
    ("System Recovery", "app-system-recovery-94f550"),
    ("System files", "app-system-files-cfb62a"),
    ("System files only · personal files kept", "app-system-files-only-personal-files-kept-0a52f8"),
    ("System files return to this restore point. Your home folder, disk encryption and package trust are preserved.", "app-system-files-return-to-this-restore-point-your-home-folder-disk--ac164a"),
    ("System restore point", "app-system-restore-point-8c2df1"),
    ("System snapshot", "app-system-snapshot-30e2de"),
    ("System snapshot {n}", "app-system-snapshot-n-0fcb26"),
    ("Terminal closed. Choose another recovery tool when you are ready.", "app-terminal-closed-choose-another-recovery-tool-when-you-are-ready-1b4f73"),
    ("The backup tool did not answer.", "app-the-backup-tool-did-not-answer-653b10"),
    ("The backup tool failed.", "app-the-backup-tool-failed-0e583e"),
    ("The backup tool is not available here.", "app-the-backup-tool-is-not-available-here-637a28"),
    ("The computer could not be restarted automatically. Use the Restart button or the power button.", "app-the-computer-could-not-be-restarted-automatically-use-the-restar-0a1d97"),
    ("The computer could not be shut down automatically. Use the power button.", "app-the-computer-could-not-be-shut-down-automatically-use-the-power--e7e9a7"),
    ("The computer restarts, asks for your disk passphrase, and restores the system automatically. Keep it connected to power.", "app-the-computer-restarts-asks-for-your-disk-passphrase-and-restores-ec55a3"),
    ("The computer will restart automatically", "app-the-computer-will-restart-automatically-ca5e63"),
    ("The disk passphrase unlocks storage. This password authorizes recovery tools.", "app-the-disk-passphrase-unlocks-storage-this-password-authorizes-rec-b5b619"),
    ("The drive can now be safely disconnected.", "app-the-drive-can-now-be-safely-disconnected-22c130"),
    ("These items will be permanently deleted. This cannot be undone.", "app-these-items-will-be-permanently-deleted-this-cannot-be-undone-57ddad"),
    ("This backup has no snapshots.", "app-this-backup-has-no-snapshots-bfa6f5"),
    ("This folder is empty", "app-this-folder-is-empty-3043f9"),
    ("Turn off this computer.", "app-turn-off-this-computer-809833"),
    ("Type exit in the terminal to return here", "app-type-exit-in-the-terminal-to-return-here-38fb5c"),
    ("Unavailable", "app-unavailable-ca1844"),
    ("Unexpected browsing authentication response.", "app-unexpected-browsing-authentication-response-48d7b2"),
    ("Unlock recovery tools", "app-unlock-recovery-tools-3c81f5"),
    ("Unlock tools", "app-unlock-tools-d96411"),
    ("Use a full-screen command line for recovery work.", "app-use-a-full-screen-command-line-for-recovery-work-ba231d"),
    ("Use an administrator account to continue", "app-use-an-administrator-account-to-continue-f275dc"),
    ("User", "app-user-b512d9"),
    ("Where is the backup?", "app-where-is-the-backup-dc186c"),
    ("Your files were restored to the folder", "app-your-files-were-restored-to-the-folder-9ab210"),
    ("Your personal files stay as they are", "app-your-personal-files-stay-as-they-are-383514"),
    ("copy", "app-copy-6f5a60"),
    ("delete", "app-delete-619759"),
    ("selected", "app-selected-d7cbbb"),
    ("{} ready to paste", "app-ready-to-paste-cf9b55"),
    ("{} selected", "app-selected-f06e57"),
];

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn bundled_messages_resolve_without_hidden_fallback() {
        let mut arguments = Arguments::new();
        let resources = include_str!("../assets/locales/en-US.ftl");
        let mut rest = resources;
        while let Some((_, tail)) = rest.split_once('$') {
            let end = tail
                .find(|c: char| !c.is_ascii_alphanumeric() && c != '-' && c != '_')
                .unwrap_or(tail.len());
            if end > 0 {
                arguments.set(&tail[..end], 2.0);
            }
            rest = tail;
        }
        for (locale, _) in &BUNDLED[1..] {
            let localizer = for_locale(&format!("{locale}_XX.UTF-8"));
            for (_, key) in MESSAGES {
                let message = localizer.resolve(key, Some(&arguments));
                assert!(
                    message.errors.is_empty(),
                    "{locale} {key}: {:?}",
                    message.errors
                );
                assert_eq!(message.locale.as_deref(), Some(*locale), "{key}");
                assert!(!message.value.is_empty(), "{locale} {key}");
            }
        }
    }
}
