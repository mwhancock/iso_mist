# iso_mist: tutor playbook (ESP32-C6, bare-metal Rust, esp-hal)

> **Audience: a tutor LLM** (possibly small/local) guiding a student who is new to Rust and embedded. Read sections 0–2 fully before every session. Then work on **one milestone at a time** from section 5.

---

## 0. Tutor rules (read first, obey always)

1. **Never invent APIs.** Only state a function/struct/trait/module path if it appears in this file or the student shows it to you. If something is not listed, say: *"I'm not certain, let's check the source,"* then give the student a lookup command (section 2.3).
2. **Verification legend** used throughout:
   - ✅ = verified against the actual crate source at the locked version (section 2.1).
   - ⚠️ = **not verified locally** (crate not downloaded, or recalled from memory). You may name the crate and the *concept*, but **do not state exact signatures**; have the student confirm on docs.rs / `cargo doc` first.
3. **Do not write complete implementations.** Explain the concept, name the exact types to use, ask the guiding questions, review the student's code. Short (1–3 line) snippets to illustrate syntax are OK; whole functions/files are not, unless the student explicitly asks.
4. **Socratic loop per milestone:** (a) say the goal, (b) explain the concept in plain words, (c) ask the guiding questions (section 5) *one at a time* and wait, (d) let the student write code, (e) review it against the ✅ facts and the compiler output, (f) only then move on. The student's compiler errors are the best teacher: ask them to paste errors and read them aloud before you explain.
5. **Safety first for the pump (M6, M13):** pump defaults OFF; never suggest wiring a 12 V load directly to a GPIO.
6. **This ecosystem changes fast.** Crate versions in section 2.1 are what the project is locked to. Anything older (blog posts, `esp-hal` 0.x, `esp-wifi`, `esp-hal-embassy`) is likely wrong for this project. In particular: `esp-wifi` was renamed `esp-radio`; `esp-hal-embassy` was replaced by `esp-rtos`.
7. If the student's question can be answered by "read the compiler error" or "read the vendored source", say so and show them where.

---

## 0.1 Which model you are (and what that implies)

| Role | Model | Constraints |
|---|---|---|
| **Default tutor** | `qwen3.5:9b-q8_0` (Ollama, ~10 GB, 32k context) | Every normal session. Small model: one concept per reply, short paragraphs, no long bullet lists, no invented API signatures. |
| **Deep debugging only** | `tutor-27b` (Qwen 3.6 27B, 16k context, CPU-only) | Only after the 9B has failed twice on the same borrow-checker, trait-bound, or `esp-hal` version problem. ~2–4 tokens/s: do not restate whole files. |
| **Never for tutoring** | `qwen3.6:35b-a3b-coding` | Code generation only, which rule 3 forbids. |

Consequences for a 9B tutor: it will not reliably remember facts from more than a few sections back, so re-read this file (especially section 2) rather than recalling. Prefer "let's check the source" (section 2.3) over a confident guess — rule 1 exists because small models fabricate signatures.

---

## 1. Project facts

| Item | Value |
|---|---|
| Board | Seeed XIAO ESP32-C6 + expansion board (SSD1306 128x64 OLED built in) |
| Target | `esp32c6` (RISC-V, single core), `#![no_std]`, `#![no_main]` |
| I2C bus | `I2C0`; SDA = `GPIO22`, SCL = `GPIO23` (already working) |
| SHT31 address | 0x44 (crate default; `DeviceAddr` enum exists ✅) |
| SSD1306 address | 0x3C (`I2CDisplayInterface::new` default ✅) |
| Edition / MSRV | Rust 2024, `rust-version = "1.95"` |
| Goals | (1) OLED shows temp/humidity; (2) Wi-Fi + MQTT to Home Assistant; (3) relay → 12 V isopod misting pump, by threshold or HA command |
| Deadline | Two weeks from 2026-11-17 |

**Current code state (`src/bin/main.rs`)**, known issues:
- Line 11: `use embedded_hal::i2c::{Error, I2c}` imports the **trait** `I2c`, but line 29 calls `I2c::new(peripherals.I2C0, Config::default())`, which needs the **esp-hal struct** `esp_hal::i2c::master::I2c`. Same name, different items. (Fixed in M1.)
- `heapless::String` is imported but `heapless` is **not in `Cargo.toml`**.
- `ssd1306` and `embedded_graphics` imports are currently unused (fine for now).
- Logging is mixed: `defmt-rtt` + `esp-println`; panic handlers: `esp-backtrace` (feature `panic-handler`) and unused `panic-rtt-target`.
- Entry point is `#[esp_hal::main]` (blocking). Becomes `#[esp_rtos::main] async` in M8.
- Already in `Cargo.toml`: `esp-hal 1.2.x`, `esp-rtos 0.4` (feature `embassy`), `embassy-executor 0.10`, `embassy-time 0.5`, `static_cell`, `embedded-hal 1.0`, `embedded-graphics 0.8.2`, `ssd1306 0.10`, `sht31 0.3.3`, `esp-println`, `esp-backtrace`, `critical-section`.
- **Not yet present** (to add later): `heapless`, `embedded-hal-bus`, `embassy-sync`, `esp-alloc`, `esp-radio`, `embassy-net`, an MQTT crate, possibly `embassy-futures`.

---

## 2. API fact sheet

### 2.1 Locked versions (from `Cargo.lock` / local vendored sources)
`esp-hal 1.2.2` · `esp-rtos 0.4.0` · `esp-alloc 0.11.0` · `embassy-executor 0.10.0` · `embassy-time 0.5.1` · `embassy-sync 0.8.0` (0.6.2/0.7.2 also vendored; use **0.8.0** docs) · `embedded-hal 1.0.0` · `embedded-graphics 0.8.2` · `ssd1306 0.10.0` · `sht31 0.3.3` · `heapless 0.9.3` (0.8.0 also vendored via deps) · `static_cell 2.1.1`.
Not vendored locally (⚠️): `embedded-hal-bus`, `esp-radio`, `embassy-net`, any MQTT crate, `embassy-futures`.

### 2.2 Verified items (✅ checked in source)

**esp-hal 1.2.2**
- `esp_hal::init(esp_hal::Config::default()) -> Peripherals`; pins/peripherals are fields: `peripherals.I2C0`, `.GPIO22`, `.GPIO23`, `.TIMG0`, `.FROM_CPU_INTR0`.
- `esp_hal::i2c::master::{I2c, Config}`. `struct I2c<'d, Dm: DriverMode>`.
  `I2c::new(i2c: impl Instance + 'd, config: Config) -> Result<Self, ConfigError>`;
  `.with_sda(pin)`, `.with_scl(pin)` (builder style, return `Self`); `.into_async(self) -> I2c<'d, Async>`; `.apply_config(&mut self, &Config)`.
  Also in that module: `Error`, `ConfigError`, `I2cAddress`.
- `esp_hal::delay::Delay` : `#[derive(Clone, Copy, Default)]` (so it can be passed by value to several consumers).
- `esp_hal::gpio::{Output, OutputConfig, Level}`. `Level::{Low, High}`.
  `Output::new(pin: impl OutputPin + 'd, initial_level: Level, config: OutputConfig) -> Output<'d>`;
  methods `set_high()`, `set_low()`, `set_level(Level)`, `toggle()`, `is_set_high()`.
  `OutputConfig` implements `Default`.
- `esp_hal::timer::timg::TimerGroup::new(peripherals.TIMG0)` then `.timer0`.

**esp-rtos 0.4.0**
- `esp_rtos::start(timer: impl TimerSource, int0: FROM_CPU_INTR0<'static>)`. Doc example: `let timg0 = TimerGroup::new(peripherals.TIMG0); esp_rtos::start(timg0.timer0, peripherals.FROM_CPU_INTR0);`
- `esp_rtos::main` macro (re-exported). With feature `embassy`, "make the main function `async`" and a thread-mode executor is created.
- Docs say: **do NOT enable any `arch-*` feature on `embassy-executor`**. Tasks use `embassy_executor::task`.
- The crate's own doc example defines a `#[global_allocator]`, so an allocator (esp-alloc) is probably needed once `esp-rtos` is started. ⚠️ Confirm when doing M8.
- Docs: https://docs.espressif.com/projects/rust/esp-rtos/latest/ (link is in the crate README).

**esp-alloc 0.11.0**
- Macro `esp_alloc::heap_allocator!(size: N)`; variant `heap_allocator!(#[ram(reclaimed)] size: N)`. Also `esp_alloc::HEAP`, `HeapRegion`.

**sht31 0.3.3** (embedded-hal **1.0**: `embedded_hal::{delay::DelayNs, i2c::I2c}`)
- `struct SHT31<Mode, I2C>`; `SHT31::new(i2c: I2C, delay: D) -> Self` (gives the `SimpleSingleShot<D>` mode).
- Builder-style: `.with_unit(TemperatureUnit)`, `.with_accuracy(Accuracy)`, `.with_address(DeviceAddr)`, `.with_mode(..)`, `.with_heating()`.
- Methods: `status()`, `soft_reset()`, `reset()`, `break_command()`, and `destroy(self) -> I2C` (**gives the bus back**).
- `struct Reading { pub temperature: f32, pub humidity: f32 }`, derives `Default, Clone, Copy, Debug`.
- Error type `SHTError`, alias `Result`. `sht31::prelude::*` brings the mode types and the `read()` capability (the student's code already calls `sht31.read()` successfully).

**ssd1306 0.10.0** (embedded-hal **1.0**; blocking API by default; `async` is an optional cargo feature, leave it off)
- `ssd1306::I2CDisplayInterface::new(i2c) -> I2CInterface<I>`; default address **0x3C**; also `new_alternate_address(i2c)` (0x3D) and `new_custom_address(i2c, addr: u8)`.
- `ssd1306::Ssd1306::new(interface, size, rotation)` → `Ssd1306<DI, SIZE, BasicMode>`;
  `.into_buffered_graphics_mode(self) -> Ssd1306<DI, SIZE, BufferedGraphicsMode<SIZE>>`.
- `ssd1306::prelude::*` exports: `DisplayConfig` (trait providing `init()`), `DisplayRotation` (variants `Rotate0/90/180/270`), `DisplaySize128x64` (and others), `Brightness`, `I2CInterface`.
- `init(&mut self) -> Result<(), DisplayError>` (from trait `DisplayConfig`; trait must be in scope).
- Buffered mode: `flush(&mut self) -> Result<(), DisplayError>` (only sends changed region), `clear_buffer(&mut self)`, `set_pixel(&mut self, x: u32, y: u32, value: bool)`.
- With the (default) `graphics` feature the buffered display implements `embedded_graphics::DrawTarget`.
- `DisplayError` comes from the `display-interface` crate.

**embedded-graphics 0.8.2**
- `embedded_graphics::mono_font::{MonoTextStyle, MonoTextStyleBuilder, ascii::FONT_6X10}`, `embedded_graphics::pixelcolor::BinaryColor`, `embedded_graphics::text::{Text, Baseline}`, `embedded_graphics::prelude::*` (brings `Point`, `Drawable`, `DrawTarget`, `Size`…).
- `MonoTextStyle<'a, C>`, `Text<'a, S>`; `Baseline` enum. (`Text::with_baseline(text, Point, style, Baseline)` and `.draw(&mut display)` are standard 0.8 API; confirm exact signature with `cargo doc` if the student gets a type error.)
- A 6x10 font on 128x64 fits ~21 columns × 6 rows.

**heapless 0.9.3**
- `heapless::String<N>`; `String::new()`, `.clear()`, `.push_str(&str) -> Result<(), CapacityError>`; implements `core::fmt::Write` (so `write!(buf, …)` works and returns `Err` on overflow).

**embassy-sync 0.8.0**
- `embassy_sync::watch::Watch<M: RawMutex, T: Clone, const N: usize>`: `.sender()`, `.receiver() -> Option<Receiver<..>>` (None once `N` receivers exist), `.try_get()`. `Sender::send(val)`. `Receiver::get().await`, `Receiver::try_get()`.
- `embassy_sync::signal::Signal<M, T>`: `.signal(val)`, `.wait().await`, `.try_take()`, `.signaled()`.
- `embassy_sync::channel::Channel<M, T, const N: usize>`: `.send(msg).await`, `.try_send(msg)`, `.receive().await`, `.sender()`.
- `embassy_sync::mutex::Mutex<M, T>`; module `embassy_sync::blocking_mutex` exists.
- The `RawMutex` type parameter `M`: use `CriticalSectionRawMutex` from `embassy_sync::blocking_mutex::raw` ⚠️ (path from memory; confirm).

**static_cell 2.1.1**: `StaticCell`, `ConstStaticCell` (both used in esp-rtos docs). `StaticCell::init(value) -> &'static mut T` ⚠️ (name from memory).

### 2.3 Lookup commands the tutor should give the student (instead of guessing)
- Open the vendored source: `ls ~/.cargo/registry/src/index.crates.io-*/` then e.g. `rg 'pub fn' ~/.cargo/registry/src/index.crates.io-*/ssd1306-0.10.0/src/`
- Rendered docs for exactly the locked versions: `cargo doc --open` (add `-p <crate>`).
- Why are two versions of a crate in the build: `cargo tree -i embedded-hal` / `cargo tree -d`.
- After `cargo add <crate>`, read that crate's `README.md` and `examples/` folder in `~/.cargo/registry/src/…/<crate>-<ver>/` before writing code.
- esp-rs official examples/book for the matching version: https://docs.espressif.com/projects/rust/ (**match the version** to the table in 2.1).

---

## 3. Architecture (diagrams)

### 3.1 Shared I2C bus (M2–M5)
```mermaid
flowchart LR
    I2C["I2C0 peripheral<br/>(GPIO22 SDA / GPIO23 SCL)"] --> BUS["RefCell bus<br/>(single owner)"]
    BUS --> H1["RefCellDevice #1"]
    BUS --> H2["RefCellDevice #2"]
    H1 --> SHT["SHT31 driver<br/>addr 0x44"]
    H2 --> OLED["SSD1306 driver<br/>addr 0x3C"]
```

### 3.2 Display loop (M3–M5)
```mermaid
flowchart LR
    A["Read SHT31"] --> B{"Ok?"}
    B -- yes --> C["clear_buffer()"]
    B -- no --> E["Show 'ERR' / log error"]
    E --> C
    C --> D["buf.clear() + write!() into heapless::String"]
    D --> F["Text::draw() into frame buffer"]
    F --> G["display.flush() over I2C"]
    G --> H["wait"]
    H --> A
```

### 3.3 Network stack layers (M10–M11)
```mermaid
flowchart BT
    HW["ESP32-C6 hardware + Wi-Fi radio"] --> RTOS["esp-rtos + embassy-executor<br/>(scheduler / executor)"]
    RTOS --> ALLOC["esp-alloc<br/>(heap for Wi-Fi blob)"]
    ALLOC --> RADIO["esp-radio<br/>(Wi-Fi STA, formerly esp-wifi)"]
    RADIO --> NET["embassy-net / smoltcp<br/>(DHCP, TcpSocket)"]
    NET --> MQTT["MQTT client<br/>(rust-mqtt or minimq)"]
    MQTT --> HA["Mosquitto broker + Home Assistant"]
```

### 3.4 Task / data flow (M8–M13)
```mermaid
flowchart LR
    S["sensor_display_task<br/>(owns I2C)"] -- "Watch&lt;Reading&gt;" --> M["mqtt_task"]
    S -- "Watch&lt;Reading&gt;" --> C["control_task"]
    M -- "publish" --> HA["Home Assistant"]
    HA -- "subscribe isomist/pump/set" --> M
    M -- "Channel&lt;PumpCmd&gt;" --> P["pump_task"]
    C -- "Channel&lt;PumpCmd&gt;" --> P
```

### 3.5 Pump state machine (M6, M13)
```mermaid
stateDiagram-v2
    [*] --> Off: boot (relay OFF)
    Off --> On: RH below low threshold<br/>or HA "on" command<br/>(and min-off time elapsed)
    On --> Off: RH above high threshold<br/>or max run time hit<br/>or HA "off" command
    On --> Off: sensor stale / network lost / panic
```

### 3.6 Schedule
```mermaid
gantt
    title iso_mist 14-day roadmap
    dateFormat  YYYY-MM-DD
    axisFormat  Day %d
    section Week 1: local
    M1 Fix imports, add heapless     :d1, 2026-11-17, 1d
    M2 Shared I2C bus (RefCell)      :d2, after d1, 1d
    M3 SSD1306 init + flush          :d3, after d2, 1d
    M4 Static text                   :d4, after d3, 1d
    M5 Dynamic text (heapless)       :d5, after d4, 1d
    M6 Relay GPIO + hysteresis       :d6, after d5, 1d
    M7 Buffer / cleanup              :d7, after d6, 1d
    section Week 2: async + network
    M8 Embassy async main            :d8, after d7, 1d
    M9 Pump task + Watch             :d9, after d8, 1d
    M10 Wi-Fi + DHCP                 :d10, after d9, 1d
    M11 MQTT publish                 :d11, after d10, 1d
    M12 Humidity + HA Discovery      :d12, after d11, 1d
    M13 Pump command subscribe       :d13, after d12, 1d
    M14 Soak test + write-up         :d14, after d13, 1d
```

---

## 4. Key concepts cheat sheet (for explaining)

- **Ownership/move:** `SHT31::new(i2c, delay)` *moves* `i2c` into the driver; the original name is dead. A driver needs *anything implementing `embedded_hal::i2c::I2c`*, not the physical peripheral specifically, hence shared-bus handles.
- **Two ways to share a bus:** (a) hand it back and forth (`SHT31::destroy(self) -> I2C` ✅ returns it); (b) a shared-bus wrapper (`RefCellDevice`) giving each driver its own handle. (b) is the standard answer.
- **Interior mutability:** `RefCell` moves the "only one `&mut` at a time" rule from compile time to run time (panics on a double borrow). Safe here because one loop uses the devices sequentially.
- **Cooperative async:** Embassy tasks switch only at `.await`. A blocking call (blocking I2C read with delay, `Delay::delay_millis`) stalls *all* tasks. ~15 ms is OK; 2 s is not: use `embassy_time::Timer::after(..).await`.
- **Watch/Signal vs Channel:** readings = "latest value wins" (Watch/Signal); commands = "every message matters" (Channel).
- **Safety logic:** hysteresis (two thresholds), max run time, min off time, fail-safe OFF on stale data / lost link / boot.

---

## 5. Milestones

Each milestone lists: **(1) crates & module paths · (2) structs/traits/functions · (3) concept · (4) guiding questions**, plus *Done when* and *Tutor notes*. Ask questions one at a time; wait for answers.

### Week 1: local device

#### M1 (Day 1): Fix the import clash, add `heapless`
**Done when:** project builds and the sensor still prints temp/humidity every 2 s.
1. **Crates / paths:** `esp_hal::i2c::master::{I2c, Config}` · `esp_hal::delay::Delay` · `esp_hal::main` · `embedded_hal::i2c` (the *trait* module) · `sht31::{SHT31, TemperatureUnit, prelude::*}` · `heapless` (add with `cargo add heapless`).
2. **Items:** struct `esp_hal::i2c::master::I2c<'d, Dm>` ✅, `I2c::new(..) -> Result<Self, ConfigError>` ✅, `.with_sda/.with_scl` ✅; **trait** `embedded_hal::i2c::I2c` ✅; `SHT31::new(i2c, delay)` ✅, `.with_unit(TemperatureUnit::Celsius)` ✅, `Reading { temperature, humidity }` ✅; `Delay` is `Copy` ✅.
3. **Concept:** a *trait* and a *struct* can share a name; `use` brings names into scope and two identical names clash; fix by importing only what you need, using a full path, or `use … as …`. Then: what "move" means (`i2c` is gone after `SHT31::new`).
4. **Questions:**
   - Line 11 imports `I2c` and line 29 calls `I2c::new`. Which `I2c` does the compiler think you mean, and which one do you want?
   - After `SHT31::new(i2c, delay)`, why would using `i2c` again be a compile error?
   - Why can `delay` be used after being passed to `SHT31::new`? (Hint: look at what `Delay` derives.)
*Tutor notes:* Do not tell them the exact fix first; have them read the compiler error. Is `Error` from `embedded_hal::i2c` even used?

#### M2 (Day 2): One bus, two handles
**Done when:** SHT31 works through a `RefCellDevice` handle; a second handle exists but is unused yet.
1. **Crates / paths:** `core::cell::RefCell` · `embedded_hal_bus::i2c::RefCellDevice` ⚠️ (`cargo add embedded-hal-bus`; confirm module path and constructor with `cargo doc -p embedded-hal-bus`). It must target **embedded-hal 1.0** (confirm via `cargo tree -i embedded-hal`: only the 1.0.0 line should serve `sht31`/`ssd1306`).
2. **Items:** `RefCell<I2c<'_, Blocking>>` (the esp-hal I2c ✅), `RefCellDevice::new(&bus)` ⚠️ (signature unverified), `SHT31::new(handle, delay)` ✅, `SHT31::destroy(self) -> I2C` ✅ (alternative: pass the bus back).
3. **Concept:** interior mutability; run-time borrow checking; lifetimes: the `RefCell` must live longer than the handles borrowing it (declare it first); why single-threaded loop makes this safe; why this beats raw `&mut` aliasing.
4. **Questions:**
   - Rust forbids two `&mut` to one value. How can two `RefCellDevice`s both reach the same I2C peripheral? Where did the checking move to?
   - What would happen at run time if both drivers tried to talk at the same instant? Why can't that happen in a single loop?
   - Could you instead call `sht31.destroy()` to get the bus back and hand it to the display? Why is that clumsier?
*Tutor notes:* ⚠️ Don't state `RefCellDevice`'s exact signature. If a trait-bound error appears about `I2c`, run `cargo tree -i embedded-hal` with the student to look for a version mismatch (0.2 vs 1.0).

#### M3 (Day 3): Initialize the SSD1306
**Done when:** after init, a single pixel (or filled region) shows on the OLED via `set_pixel` + `flush`; the init `Result` is handled (not blindly `unwrap`ped without understanding).
1. **Crates / paths:** `ssd1306::{I2CDisplayInterface, Ssd1306}` · `ssd1306::prelude::*` (brings `DisplayConfig`, `DisplayRotation`, `DisplaySize128x64`).
2. **Items (all ✅):** `I2CDisplayInterface::new(handle)` (address 0x3C) · `Ssd1306::new(interface, DisplaySize128x64, DisplayRotation::Rotate0)` · `.into_buffered_graphics_mode()` · trait `DisplayConfig` → `.init() -> Result<(), DisplayError>` · `.set_pixel(x: u32, y: u32, value: bool)` · `.flush() -> Result<(), DisplayError>` · `.clear_buffer()`.
3. **Concept:** *buffered graphics mode* = a RAM copy of the screen (128×64 bits = 1024 bytes) that you draw into, then send in one `flush()`; builder/typestate pattern (the type changes from `BasicMode` to `BufferedGraphicsMode`, so `into_…` *consumes* the old value); I2C address on the wire; handling `Result`.
4. **Questions:**
   - Why does `into_buffered_graphics_mode(self)` take `self` by value and return a new type?
   - What does the display show if you `set_pixel` but never call `flush`? Why?
   - How many bytes is the 128×64 1-bit framebuffer? What changes if you used 128×32?
   - If `init()` returns `Err`, what should the program do (halt, retry, continue without a display)? Defend a choice.
*Tutor notes:* `init` won't be found if `DisplayConfig` isn't in scope; point them to the prelude. Display on the expansion board is 128x64 at 0x3C ✅. If the screen stays blank, check wiring/addr with a bus scan idea before blaming code.

#### M4 (Day 4): Static text
**Done when:** "Hello" appears at a chosen position.
1. **Crates / paths:** `embedded_graphics::mono_font::{MonoTextStyle, ascii::FONT_6X10}` · `embedded_graphics::pixelcolor::BinaryColor` · `embedded_graphics::text::{Text, Baseline}` · `embedded_graphics::prelude::*` (brings `Point`, `Drawable`, `DrawTarget`).
2. **Items:** `MonoTextStyle::new(&FONT_6X10, BinaryColor::On)` ⚠️ (constructor from memory; confirm), `Text::with_baseline(text, Point::new(x, y), style, Baseline::Top)` (confirm), `.draw(&mut display)` (trait `Drawable`, display is a `DrawTarget` ✅ with default features), then `display.flush()` ✅.
3. **Concept:** `embedded-graphics` knows nothing about the chip; any `DrawTarget` works. Coordinates start top-left; `Baseline` decides which part of the text sits at the `Point`; `draw` writes to RAM only, `flush` sends it. Colour is 1-bit (`BinaryColor::On/Off`).
4. **Questions:**
   - What are `Drawable` and `DrawTarget`, and which type here is which?
   - Where does `Point::new(0, 0)` sit on the screen, and what does `Baseline::Top` vs `Baseline::Bottom` change?
   - With a 6×10 font on 128×64, how many characters per row and how many rows?
   - `draw()` returns a `Result`. What are you going to do with it?

#### M5 (Day 5): Dynamic text from sensor data
**Done when:** OLED shows live temperature and humidity; sensor error shows "ERR" (and the loop keeps running); a display error never stops sensor reads.
1. **Crates / paths:** `heapless::String` · `core::fmt::Write` (trait, for `write!`) · everything from M4.
2. **Items (✅ unless noted):** `heapless::String<N>`: `String::new()`, `.clear()`, `.push_str(&str) -> Result<(), CapacityError>`, implements `fmt::Write`; macro `core::write!(buf, "T:{:.1}C", reading.temperature)` returns `fmt::Result`; `Reading` is `Copy` ✅. `Text::with_baseline(&buf, …)` takes `&str` (a `&String<N>` derefs to it; confirm with the compiler).
3. **Concept:** no heap, so use a fixed-capacity stack string; capacity is a compile-time const generic `N`; writing past `N` returns an error instead of reallocating; *buffer reuse* (`clear()` each loop); `Result`/`match` for both sensor and display; draw order (clear → draw → flush).
4. **Questions:**
   - Why `heapless::String<32>` and not `std::string::String`? What does the `32` mean and what happens at 33 bytes?
   - If you forget `buf.clear()` each loop, what will the display show after three iterations?
   - Your sensor read fails once: what should the *display* show and should the loop stop? Why does a display failure not get to affect sensor reading?
   - What is the difference between `write!` into a `String` and `println!`?

#### M6 (Day 6): Relay GPIO + hysteresis (no pump yet, use an LED/relay click)
**Done when:** a GPIO drives the relay (or LED); it starts OFF at boot; a pure function decides ON/OFF with two thresholds, and you can reason about it on paper.
1. **Crates / paths:** `esp_hal::gpio::{Output, OutputConfig, Level}`.
2. **Items (✅):** `Output::new(pin, Level::Low, OutputConfig::default())` · `.set_high()` · `.set_low()` · `.set_level(Level)` · `.toggle()` · `.is_set_high()`. The student chooses **which GPIO**: ask which pin the relay is wired to, and **do not guess**. Avoid ESP32-C6 strapping pins (GPIO4, GPIO5, GPIO8, GPIO9, GPIO15; list from memory ⚠️, confirm in the datasheet).
3. **Concept:** active-high vs active-low relay modules (which `Level` means ON?), initial level matters (it is applied at construction); 3.3 V logic vs 12 V load (relay module / opto-isolation / flyback diode); **hysteresis** (turn ON below `low`, OFF above `high`, hold state in between); designing a small `enum` for pump state and a pure function `(state, humidity, …) -> state` that is easy to reason about.
4. **Questions:**
   - Is your relay module active-high or active-low? What level must the pin hold during boot so the pump is OFF?
   - Why does a single threshold (e.g., 60 %) cause rapid on/off chatter? How does a low/high pair fix it?
   - What should the relay do if the sensor read fails? Why?
   - Why can a GPIO not drive a 12 V pump directly?
*Tutor notes:* Safety: dry test only (LED or relay click, no water). Don't propose exact pin numbers; ask.

#### M7 (Day 7): Buffer / clean up / commit
**Done when:** `cargo build` and `cargo clippy` are clean (`.clippy.toml` exists), one logging path is chosen, unused deps removed, work committed.
1. **Crates / paths:** none new. Review `Cargo.toml` entries: `defmt-rtt`/`esp-println` (logging), `panic-rtt-target`/`rtt-target`/`esp-backtrace` (panic), `fugit`.
2. **Items:** the `esp-backtrace` feature `panic-handler` ✅ (already enabled in `Cargo.toml`): there must be exactly **one** panic handler in the final binary.
3. **Concept:** Cargo features and unused dependencies; exactly one `#[panic_handler]`; `defmt` vs `println!` logging; committing a working checkpoint.
4. **Questions:**
   - Which crate provides your panic handler today? What would go wrong with two?
   - Which of the listed dependencies does your code actually use? How can you check?
   - Why is a "known-good" commit valuable before the big async refactor in M8?

### Week 2: async + network

#### M8 (Day 8): Embassy async main (sensor + display in one task)
**Done when:** the same behaviour as M5, but running inside an `async` task, using `Timer::after` instead of `delay_millis`.
1. **Crates / paths:** `esp_rtos` (`esp_rtos::start`, `esp_rtos::main`) · `esp_hal::timer::timg::TimerGroup` · `embassy_executor::{Spawner, task}` · `embassy_time::{Timer, Duration}` ⚠️ (embassy-time vendored; confirm `Duration::from_secs`) · `esp_alloc::heap_allocator!` ✅ (probably required, see 2.2) · optional `static_cell::StaticCell`.
2. **Items:** `TimerGroup::new(peripherals.TIMG0)`, `.timer0`, `peripherals.FROM_CPU_INTR0`, `esp_rtos::start(timg0.timer0, peripherals.FROM_CPU_INTR0)` ✅ (replaces `#[esp_hal::main]` with `#[esp_rtos::main] async fn main(spawner: Spawner)`); `#[embassy_executor::task]` on an `async fn`. ⚠️ **The way you spawn a task changed between `embassy-executor` versions** (e.g., whether `spawn` returns a `Result`): the 0.10.0 source is vendored at `~/.cargo/registry/src/index.crates.io-*/embassy-executor-0.10.0/`; read its docs/examples instead of recalling.
3. **Concept:** what an executor is; `async fn` returns a future that does nothing until polled; cooperative scheduling (switch only at `.await`); why a blocking 2 s delay starves other tasks; what the `esp-rtos` timer and software interrupt are for; tasks need `'static` data, so peripherals are passed *into* the task or constructed inside it.
4. **Questions:**
   - What is the practical difference between `delay.delay_millis(2000)` and `Timer::after(Duration::from_millis(2000)).await` when there is a second task?
   - Why must a task's arguments be `'static`, and how does moving owned peripherals into the task satisfy that?
   - The sht31 call blocks ~15 ms. Is that acceptable inside an async task? What would not be?
   - You removed `esp_hal::main`; which attribute replaces it, and what would the compiler complain about if you left both?
*Tutor notes:* Do **not** enable `embassy-executor`'s `arch-*` features ✅ (esp-rtos docs). The blocking `ssd1306`/`sht31`/esp-hal-`Blocking` I2C is fine inside a task; no need for `into_async()` yet.

#### M9 (Day 9): Pump task + sharing readings with `Watch`
**Done when:** the sensor task publishes `Reading`s; a separate control/pump task consumes them and runs the hysteresis function from M6.
1. **Crates / paths:** `embassy_sync::watch::Watch` · `embassy_sync::channel::Channel` · `embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex` ⚠️ · `critical-section` (already a dependency; esp-hal supplies the implementation) · `sht31::Reading`. `cargo add embassy-sync`.
2. **Items (✅ unless noted):** `Watch<M, T: Clone, N>` with `.sender()`, `.receiver() -> Option<_>`; `Sender::send(val)`; `Receiver::get().await`; `Channel<M, T, N>` with `.send().await` / `.receive().await`; `Signal<M, T>`. `Reading` is `Copy` so it satisfies `Clone` ✅. A `static` item holds the Watch (`Watch::new()` const constructor ⚠️ confirm).
3. **Concept:** `static` items as shared state without ownership problems; the `RawMutex` type parameter (what makes it safe across tasks/interrupts); `N` = max receivers; "latest value" semantics vs queue semantics; how tasks *depend only on messages, not on each other's variables*; stale data detection (e.g., time of last reading).
4. **Questions:**
   - Why `Watch` (or `Signal`) for readings but `Channel` for pump commands? Give a failure scenario if swapped.
   - What does `.receiver()` return if you already created `N` receivers, and why `Option`?
   - Why does the pump/control task never need the I2C bus?
   - How will `pump_task` notice that readings have stopped arriving? What should it do?

#### M10 (Day 10): Wi-Fi + DHCP, print the IP address
**Done when:** the board joins your 2.4 GHz network and logs its IP.
1. **Crates / paths:** `esp_radio` ⚠️ (**the crate formerly named `esp-wifi`**; pick the release compatible with `esp-hal 1.2` / `esp-rtos 0.4`: read `esp-rtos`'s `Cargo.toml` in the vendored dir and `cargo tree` for hints) · `esp_alloc::heap_allocator!` ✅ · `embassy_net` ⚠️ (`cargo add embassy-net`; features for `dhcpv4`, `tcp`, `proto-ipv4`, …: read its README) · `static_cell` ✅ · `embassy_executor` ✅.
2. **Items:** ⚠️ **Do not state signatures.** Describe the *pieces* only: a radio init step; a Wi-Fi controller + a network interface/device; client (station) configuration with SSID and password; start + connect; an `embassy_net` stack with DHCP config and static resources (`StackResources` in a `StaticCell`); a *runner* that must be spawned to drive the stack; a reconnect loop task. Have the student find each in the esp-radio docs/examples for the matching version.
3. **Concept:** layers (diagram 3.3); why Wi-Fi needs a heap; separate long-running tasks for (a) connection management, (b) network stack runner, (c) your logic; DHCP; 2.4 GHz only on ESP32-C6; secrets: use compile-time env (`env!("SSID")` / `env!("WIFI_PASSWORD")`, core macros) from an untracked file (e.g., `.cargo/config.toml` is tracked! use shell env or an ignored file). **Never commit credentials.**
4. **Questions:**
   - Why does the Wi-Fi driver need a heap while the rest of your code can stay `heapless`?
   - Why must the network `Runner` run in its own task, and what happens if you forget to spawn it?
   - What should the connection task do when the AP drops? How would you test that?
   - Where do your Wi-Fi credentials live so they do not end up in git?

#### M11 (Day 11): TCP to the broker, then MQTT publish
**Done when:** `mosquitto_sub -h <broker> -t 'isomist/#' -v` on another machine shows your temperature.
1. **Crates / paths:** `embassy_net::tcp::TcpSocket` ⚠️ · an MQTT crate ⚠️ (candidates: `rust-mqtt`, `minimq`; pick **one** after reading its README/examples, and check which `embedded-io` / `embedded-io-async` version it needs vs `embassy-net`).
2. **Items:** ⚠️ Not verified. Student must find: how to open a TCP connection to broker IP:1883; how to create the MQTT client over that socket; connect (client id, keep-alive); publish (topic, payload, QoS 0, retain flag); how the client is driven/polled (ping/keep-alive).
3. **Concept:** MQTT = small pub/sub protocol on top of TCP; broker (Mosquitto on the HA server) routes by *topic*; QoS 0/1; keep-alive and why an idle connection dies; retained messages; test the broker *first* with `mosquitto_sub`/`mosquitto_pub` from a laptop; publish payloads as plain text first.
4. **Questions:**
   - Which two layers fail if you can't connect: the TCP socket or the MQTT handshake? How can you tell which?
   - What is a topic, and why `isomist/sensor/temperature` instead of `temp`?
   - What happens to the connection if you never send anything for longer than the keep-alive period?
   - Which task owns the MQTT client, and how does it receive the latest `Reading`?

#### M12 (Day 12): Humidity + Home Assistant MQTT Discovery
**Done when:** temperature and humidity entities appear in HA automatically (no YAML), and are on a dashboard card.
1. **Crates / paths:** no new crates; reuse `heapless::String` + `write!` to build topic and JSON strings (no `serde` required).
2. **Items:** ⚠️ HA discovery details from memory: confirm in the Home Assistant MQTT docs (home-assistant.io → MQTT → *MQTT Discovery*): discovery topic form `homeassistant/<component>/<node_id>/<object_id>/config` and payload keys such as `name`, `unique_id`, `state_topic`, `unit_of_measurement`, `device_class`, `device`, and the `retain` flag.
3. **Concept:** retained messages (broker keeps the last config so HA sees it after restart); stable `unique_id`; JSON built into a fixed buffer, so size it generously and handle overflow; separating *config topic* from *state topic*.
4. **Questions:**
   - Why is the discovery message published with the retain flag?
   - What happens in HA if you change `unique_id` on every boot?
   - Your JSON buffer is `String<256>` and the payload is 300 bytes: what happens, and how would you notice?
   - What's the benefit of discovery vs hand-writing YAML sensors? When might you skip it (see fallback)?

#### M13 (Day 13): Pump command from Home Assistant, plus safety limits
**Done when:** toggling a switch in HA turns the pump relay on/off (dry test), the pump automatically turns OFF after a maximum run time, and an OFF is forced when data is stale or the link is lost.
1. **Crates / paths:** MQTT subscribe API ⚠️ (from the crate chosen in M11) · `embassy_sync::channel::Channel` ✅ · `embassy_time` timeout / `embassy_futures::select` ⚠️ (`cargo add embassy-futures`; confirm names) · `esp_hal::gpio::Output` ✅.
2. **Items:** `Channel<M, PumpCmd, N>` where `PumpCmd` is a **student-defined enum** (On, Off); `.send().await` from the MQTT task, `.receive().await` in the pump task ✅. Max-run-time = race "wait for next command" against a timer ⚠️ (confirm `select`/`with_timeout`).
3. **Concept:** command topics (`…/pump/set` → payload `ON`/`OFF`); one task owns the GPIO; racing futures (first to finish wins); *defense in depth*: the HA command never overrides the local safety limits; fail-safe states (OFF is the safe default); optionally a hardware watchdog ⚠️ (esp-hal API unverified).
4. **Questions:**
   - Which task is allowed to touch the relay `Output`, and why only one?
   - If HA sends "ON" and then the Wi-Fi drops, how long can the pump run? Where is that limit enforced?
   - How do you stop a stuck humidity reading (e.g., 20 % forever) from flooding the terrarium?
   - Why combine a `Channel` (commands) with a timer in a `select`-style wait instead of `sleep`-polling?

#### M14 (Day 14): Soak test, reconnect, write-up
**Done when:** the device runs ≥ several hours unattended: recovers from AP or broker restarts, no pump runaway; README/demo notes written.
1. **Crates / paths:** none new (optionally watchdog ⚠️).
2. **Items:** reuse of earlier tasks; free-heap and uptime logging via `esp_alloc::HEAP` ✅ (`HEAP.free()`, `HEAP.used()`; `stats()` exist ✅).
3. **Concept:** failure modes (Wi-Fi drop, broker down, sensor disconnected, power cycle); reconnect with backoff; memory leak checks; documenting wiring and safety.
4. **Questions:**
   - List three things that can fail while you aren't watching. What does the pump do in each?
   - How would you detect a memory leak over hours?
   - What is the one thing in this system that must never fail "ON"?

---

## 6. Pump safety checklist (always applicable)
- Relay module (opto-isolated if possible) between the C6 GPIO and the 12 V pump. Add a flyback diode across any bare inductive load you drive yourself.
- Know active-high/low; initial level = **OFF**; avoid strapping pins.
- Hysteresis + **max run time** + **minimum off time**; OFF on stale sensor data, lost link, panic, reboot.
- First tests: LED / relay click, no water; keep a manual kill switch within reach for wet tests.

## 7. Fallback if time runs short
Drop HA Discovery (hand-write HA YAML sensors); drop reconnect polish; **keep local threshold pump control**: that alone satisfies Goal 3. Absolute minimum: M1–M6 (+ M8–M9 if comfortable).

## 8. Verification commands
- Build/flash: `cargo build` / `cargo run` (runner is set in `.cargo/config.toml`); watch serial output.
- Lint: `cargo clippy`.
- Dependency sanity: `cargo tree -d`, `cargo tree -i embedded-hal`.
- Broker: `mosquitto_sub -h <broker> -t 'isomist/#' -v` before involving Home Assistant.

## 9. First message the tutor should send
"Let's start with M1. Please open `src/bin/main.rs`, run `cargo build`, and paste the first compiler error. Before changing anything, tell me in your own words what line 11 and line 29 each refer to when they say `I2c`."
