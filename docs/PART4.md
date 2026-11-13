# Rimflux Part 4

## Cara pakai

- Klik permukaan notch untuk membuka atau menutup tampilan mode saat ini. Tombol Play/Pause dan timeline musik punya area interaksi sendiri.
- Klik kanan notch atau klik tombol **⋮** di tampilan expanded. Pilih **Clock**, **Music**, atau **System Monitor**. Saat notch fokus, **F10** membuka menu dan **1**, **2**, **3** memilih mode.
- Gunakan **Auto Mode** untuk mengikuti aktivitas musik. **Pin Current Mode** menyimpan mode yang dipilih untuk peluncuran berikutnya.
- Di Windows, buka ikon Rimflux di notification area untuk memilih mode, mengganti kebijakan visibilitas, membuka Settings, atau keluar. Ikon bisa berada di tray overflow.
- **Ctrl+Alt+R** menampilkan kembali notch yang disembunyikan. Di atas browser maximized, shortcut membuka peek selama sekitar delapan detik tanpa mengambil fokus. Tekan shortcut lagi untuk menutupnya, atau tekan **Esc** saat notch fokus.
- Pilih **Hide Temporarily** untuk sembunyi selama 15 menit, satu jam, atau sampai ditampilkan kembali. Gunakan **Quit Rimflux** untuk menghentikan proses.

## Audit dan perubahan UI

Sebelumnya, pilihan mode terutama bergantung pada shortcut inspeksi dan aktivitas media. Klik Clock juga bisa memilih Music saat sesi tersedia. Adapter Windows sudah memosisikan jendela secara top-center dan membatasi input melalui region notch. Play/Pause Compact sudah memakai hitbox 24 × 24 px dan menangkap klik sampai tombol dilepas. Pengaturan berasal dari TOML dan belum mempunyai menu simpan, tray, atau shortcut global.

Part 4 menambah satu tombol overflow pada ketiga tampilan expanded. Menu dan Settings memakai panel dalam jendela notch yang sama. Ketiga mode memiliki ikon dan tanda centang di menu. Clock Compact tetap 116 × 27 dengan waktu di tengah. Clock Expanded menampilkan hari dan tanggal lengkap. System Monitor memakai data CPU, RAM, dan jaringan dari layanan sistem yang sudah ada.

Music Compact tetap 345 × 42. Music Expanded tetap 456 × 129 karena susunan tombol yang ada mencapai bagian bawah panel. Tombol overflow memakai ruang More yang sudah ada. Pemilihan mode mempertahankan sesi media, dan perpindahan tampilan memakai transisi notch yang sudah ada. Capture audio dan animasi waveform berhenti saat tampilannya tidak dipakai.

## State dan visibilitas

`NotchMode`, `NotchExpansion`, `ModeSelection`, dan `VisibilityPolicy` memiliki state terpisah. Pemilihan mode eksplisit mempertahankan ekspansi. Manual dan Pinned mengabaikan pemicu Auto. Auto memilih Music Compact saat playback mulai dan kembali ke Clock setelah sesi berhenti melewati `music.idle_delay_secs`. Pause mempertahankan metadata musik.

Runtime membedakan visible, automatic hide beserta penyebabnya, manual hide beserta waktu berakhirnya, dan peek. Manual hide bertahan saat foreground berubah. Jika fullscreen protection aktif, peek tidak menimpa fullscreen. **Always Visible** mengabaikan kondisi maximized dan tetap mengikuti sakelar fullscreen. **Desktop Only** menyembunyikan notch saat foreground bukan desktop. **Smart** menyembunyikan notch di atas browser maximized secara bawaan. Debounce bawaan 200 ms untuk hide dan 350 ms untuk restore. Jika tray dan hotkey sama-sama tidak tersedia, notch tetap dapat diakses.

Jendela Windows benar-benar disembunyikan agar tidak mencegat klik. Reveal mempertahankan pusat notch dan tidak memindahkan atau mengubah ukuran aplikasi lain. Panel dibuka dan ditutup langsung. Mode dan expand/collapse tetap mengikuti pengaturan reduced motion.

## Integrasi Windows

Worker Win32 terpisah memakai notifikasi foreground, minimize, dan perubahan geometri, dengan rekonsiliasi tiap satu detik. Jendela Rimflux sendiri, taskbar, dan popup sementara tidak mengganti klasifikasi foreground yang terakhir valid. Klasifikasi menggunakan window placement, batas DWM, monitor notch, dan nama executable browser yang dikenal. Fullscreen memakai batas monitor dan membedakan jendela maximized biasa yang masih memiliki dekorasi. [SetWinEventHook](https://learn.microsoft.com/en-us/windows/desktop/api/winuser/nf-winuser-setwineventhook)

Tray menggunakan `Shell_NotifyIconW` dan mencoba mendaftarkan kembali ikon setelah pesan `TaskbarCreated`. Global shortcut memakai `RegisterHotKey` dengan `MOD_NOREPEAT`. Konflik registrasi dilaporkan di menu dan Settings, dan shortcut lama tetap aktif selama sesi berjalan jika shortcut baru gagal. Quit melepas event hook, tray, dan registrasi shortcut. [Shell_NotifyIconW](https://learn.microsoft.com/en-us/windows/win32/api/shellapi/nf-shellapi-shell_notifyiconw) · [RegisterHotKey](https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-registerhotkey)

## Pengaturan

Settings menyimpan navigasi, visibilitas, interaksi, reduced motion, dan preferensi tray ke jalur TOML yang sudah dipakai Rimflux. Simpan memvalidasi input dan mengganti file melalui file sementara di direktori yang sama. Nilai konfigurasi musik tetap ikut tersimpan. Komentar dan format file bisa berubah saat TOML ditulis ulang. File yang rusak tetap tersedia untuk diperbaiki. **Start with Windows** mengubah entri Run per pengguna dari Settings. [Run and RunOnce Registry Keys](https://learn.microsoft.com/en-us/windows/win32/setupapi/run-and-runonce-registry-keys)

Lihat [config/default.toml](../config/default.toml) untuk pilihan dan nilai bawaan. Konfigurasi lama tetap membaca `notch.default_mode` dan `music.auto_switch` jika opsi navigasi baru belum diisi.

## File Part 4

| Bagian | File |
| --- | --- |
| Mode, visibilitas, dan penyimpanan | `src/core/desktop.rs`, `state.rs`, `config.rs`, `errors.rs`, `mod.rs`, `config/default.toml` |
| Runtime dan desktop | `src/app.rs`, `src/message.rs`, `src/platform/windows_desktop.rs`, `windows.rs`, `mod.rs`, `traits.rs`, `linux.rs`, `Cargo.toml` |
| Tampilan | `src/ui/desktop.rs`, `layout.rs`, `mod.rs`, `src/modules/clock.rs` |
| Tes dan penggunaan | `tests/desktop.rs`, `tests/state_transitions.rs`, `README.md`, dokumen ini |

## Cakupan desktop

Navigasi dan Settings memakai kode bersama. Deteksi foreground, tray, hotkey global, dan autostart Part 4 tersedia di adapter Windows. Klasifikasi fullscreen berbasis geometri tetap memerlukan QA dengan aplikasi yang dipakai sehari-hari. Perilaku Explorer restart, sleep/wake, scaling 100–150%, browser, dan Apple Music langsung perlu diuji pada desktop. Peek ditutup saat fokus berpindah dari notch atau ketika waktu inaktif habis.

## Verifikasi

- `cargo fmt --all -- --check`, `cargo check --locked --offline -j 1 --all-targets`, dan Clippy dengan `-D warnings` lolos di Windows.
- `cargo test --locked --offline -j 1 --all-targets` lulus 47 tes, termasuk 11 tes Part 4. Tes meliputi transisi mode, prioritas visibilitas, debounce, peek, persistensi TOML, dan pemisahan event Play/Pause.
- `cargo build --locked --offline -j 1 --release` menghasilkan `target/release/rimflux.exe`. Saat build di sandbox, linker memakai direktori temp dalam `target` agar dapat mengganti arsip sementara secara atomik.
- Peluncuran normal di sesi Windows mencatat tray dan hotkey aktif serta mendeteksi sesi Apple Music. Verifikasi ini berdasarkan log startup. Interaksi dengan desktop, pemutaran dan seeking Apple Music, serta runtime Wayland belum diuji dalam pekerjaan ini.
