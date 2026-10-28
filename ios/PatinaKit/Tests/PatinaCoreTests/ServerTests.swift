// Port of desktop/core/ops/server.test.ts.

import PlexKit
import Testing

@testable import PatinaCore

private func pref(_ id: String, _ group: String, _ kind: String) -> Ops.Server.Pref {
    Ops.Server.Pref(id: id, label: id, summary: nil, kind: kind, value: "", options: [], advanced: false, group: group)
}

@Suite struct ServerTests {
    @Test func groupsKeepPlexOrderWithUngroupedLast() {
        let groups = Ops.Server.grouped([
            pref("Odd", "", "text"),
            pref("FriendlyName", "general", "text"),
            pref("TranscoderQuality", "transcoder", "int"),
            pref("LogVerbose", "general", "bool"),
            pref("DlnaEnabled", "dlna", "bool"),
        ])
        #expect(groups.map(\.label) == ["General", "Transcoder", "DLNA", "Other"])
        #expect(groups[0].prefs.count == 2)
    }

    @Test func butlerSwitchesJoinTheirTasks() {
        let m = Ops.Server.joinButler(
            [ButlerTask(name: "BackupDatabase"), ButlerTask(name: "CheckForUpdates")],
            [
                pref("FriendlyName", "general", "text"),
                pref("ButlerStartHour", "butler", "int"),
                pref("ButlerTaskBackupDatabase", "butler", "bool"),
                pref("ButlerDatabaseBackupPath", "butler", "text"),
            ])
        #expect(m.tasks[0].pref?.id == "ButlerTaskBackupDatabase")
        #expect(m.tasks[1].pref == nil)
        #expect(m.tasks[1].name == "CheckForUpdates")  // the task's own fields read through
        #expect(m.schedule.map(\.id) == ["ButlerStartHour", "ButlerDatabaseBackupPath"])
    }
}
