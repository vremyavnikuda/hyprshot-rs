use anyhow::{Context, Result};
use log::{info, debug, trace};
use std::os::unix::io::AsRawFd;
use std::os::fd::BorrowedFd;
use wayland_client::{
    protocol::{wl_registry, wl_shm, wl_output, wl_buffer, wl_shm_pool},
    Connection, Dispatch, QueueHandle,
};
use wayland_protocols_wlr::screencopy::v1::client::{
    zwlr_screencopy_frame_v1,
    zwlr_screencopy_manager_v1::ZwlrScreencopyManagerV1,
};
use memmap2::MmapMut;
use std::io::Cursor;
use png::{Encoder, ColorType, BitDepth};

pub struct WaylandScreenshot {
    _conn: Connection,
    event_queue: wayland_client::EventQueue<State>,
    state: State,
}

struct State {
    screencopy_manager: Option<ZwlrScreencopyManagerV1>,
    shm: Option<wl_shm::WlShm>,
    outputs: Vec<wl_output::WlOutput>,
    frame_state: FrameState,
    debug: bool,
}

struct FrameState {
    _buffer: Option<MmapMut>,
    width: u32,
    height: u32,
    stride: u32,
    done: bool,
    failed: bool,
    buffer_done: bool,
}

impl WaylandScreenshot {
    pub fn new(debug: bool) -> Result<Self> {
        debug!("Initializing Wayland screenshot");
        let conn = Connection::connect_to_env()?;
        let display = conn.display();
        let mut event_queue = conn.new_event_queue();
        let qh = event_queue.handle();

        let mut state = State {
            screencopy_manager: None,
            shm: None,
            outputs: Vec::new(),
            frame_state: FrameState {
                _buffer: None,
                width: 0,
                height: 0,
                stride: 0,
                done: false,
                failed: false,
                buffer_done: false,
            },
            debug,
        };

        let _registry = display.get_registry(&qh, ());
        debug!("Registry created, waiting for protocols...");

        // Wait for all required protocols to be initialized
        let mut retries = 0;
        while (state.screencopy_manager.is_none() || state.shm.is_none() || state.outputs.is_empty()) && retries < 5 {
            if debug {
                info!("Retry {}/5: Waiting for protocols...", retries + 1);
            }
            event_queue.roundtrip(&mut state)?;
            retries += 1;
        }

        if state.screencopy_manager.is_none() {
            return Err(anyhow::anyhow!(
                "Screencopy manager not available. Make sure your compositor supports the wlr-screencopy protocol."
            ));
        }

        debug!("Wayland initialization complete");
        debug!("Found {} outputs", state.outputs.len());
        debug!("Screencopy manager: {:?}", state.screencopy_manager.is_some());
        debug!("SHM: {:?}", state.shm.is_some());

        Ok(Self { _conn: conn, event_queue, state })
    }

    fn encode_as_png(&self, data: &[u8], width: u32, height: u32) -> Result<Vec<u8>> {
        let mut png_data = Vec::new();
        {
            let mut encoder = Encoder::new(Cursor::new(&mut png_data), width, height);
            encoder.set_color(ColorType::Rgba);
            encoder.set_depth(BitDepth::Eight);
            
            let mut writer = encoder.write_header()
                .context("Failed to write PNG header")?;
                
            writer.write_image_data(data)
                .context("Failed to write PNG data")?;
        }
        Ok(png_data)
    }

    pub fn capture_region(&mut self, x: i32, y: i32, width: u32, height: u32) -> Result<Vec<u8>> {
        debug!("Capturing region: {}x{} at ({},{})", width, height, x, y);
        let stride = width * 4; // 4 bytes per pixel (RGBA)
        let size = (stride * height) as i32;
        
        debug!("Creating shared memory buffer: {} bytes", size);
        let file = tempfile::tempfile().context("Failed to create temporary file for shared memory")?;
        file.set_len(size as u64).context("Failed to set temporary file size")?;

        let mmap = unsafe {
            MmapMut::map_mut(&file).context("Failed to memory map the file")?
        };

        let shm = self.state.shm.as_ref().unwrap().clone();
        let pool = shm.create_pool(unsafe { BorrowedFd::borrow_raw(file.as_raw_fd()) }, size, &self.event_queue.handle(), ());
        debug!("Created shared memory pool");

        let formats = [
            wl_shm::Format::Xrgb8888,
            wl_shm::Format::Argb8888,
            wl_shm::Format::Xbgr8888,
            wl_shm::Format::Abgr8888,
        ];

        let mut _buffer = None;
        for format in formats.iter() {
            if self.state.debug {
                info!("Trying format: {:?}", format);
            }
            
            _buffer = Some(pool.create_buffer(
                0,
                width as i32,
                height as i32,
                stride as i32,
                *format,
                &self.event_queue.handle(),
                (),
            ));
            debug!("Created buffer with format {:?}", format);

            let frame = self.state.screencopy_manager.as_ref().unwrap()
                .capture_output_region(0, &self.state.outputs[0], x, y, width as i32, height as i32, &self.event_queue.handle(), ());
            debug!("Requested frame capture");
            frame.copy(_buffer.as_ref().unwrap());

            // Wait for buffer data and frame completion
            let mut timeout = 0;
            while !self.state.frame_state.done && !self.state.frame_state.failed && !self.state.frame_state.buffer_done {
                if self.state.debug && timeout % 10 == 0 {
                    info!("Waiting for frame capture... (attempt {})", timeout + 1);
                }
                self.event_queue.blocking_dispatch(&mut self.state)?;
                timeout += 1;
                if timeout > 50 { // 5 seconds timeout
                    debug!("Frame capture timeout");
                    break;
                }
                std::thread::sleep(std::time::Duration::from_millis(100));
            }

            if !self.state.frame_state.failed {
                debug!("Frame capture successful");
                break;
            }

            if self.state.debug {
                info!("Format {:?} failed, trying next format", format);
            }
        }

        if self.state.frame_state.failed {
            return Err(anyhow::anyhow!("Frame capture failed - no supported buffer format found"));
        }

        debug!("Frame capture complete, encoding as PNG");
        let png_data = self.encode_as_png(&mmap, width, height)?;
        debug!("PNG encoding complete, size: {} bytes", png_data.len());
        Ok(png_data)
    }
}

impl Dispatch<wl_registry::WlRegistry, ()> for State {
    fn event(
        state: &mut Self,
        registry: &wl_registry::WlRegistry,
        event: wl_registry::Event,
        _: &(),
        _: &Connection,
        qh: &QueueHandle<Self>,
    ) {
        if let wl_registry::Event::Global { name, interface, version } = event {
            if state.debug {
                info!("Global event: interface={} name={} version={}", interface, name, version);
            }
            match interface.as_str() {
                "zwlr_screencopy_manager_v1" => {
                    if state.screencopy_manager.is_none() {
                        let screencopy_manager = registry.bind::<ZwlrScreencopyManagerV1, _, _>(
                            name,
                            3,
                            qh,
                            (),
                        );
                        state.screencopy_manager = Some(screencopy_manager);
                    }
                }
                "wl_shm" => {
                    if state.shm.is_none() {
                        let shm = registry.bind::<wl_shm::WlShm, _, _>(
                            name,
                            1,
                            qh,
                            (),
                        );
                        state.shm = Some(shm);
                    }
                }
                "wl_output" => {
                    let output = registry.bind::<wl_output::WlOutput, _, _>(
                        name,
                        3,
                        qh,
                        (),
                    );
                    state.outputs.push(output);
                }
                _ => {}
            }
        }
    }
}

impl Dispatch<ZwlrScreencopyManagerV1, ()> for State {
    fn event(
        _state: &mut Self,
        _proxy: &ZwlrScreencopyManagerV1,
        _event: <ZwlrScreencopyManagerV1 as wayland_client::Proxy>::Event,
        _data: &(),
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
    ) {
    }
}

impl Dispatch<wl_shm::WlShm, ()> for State {
    fn event(
        _state: &mut Self,
        _proxy: &wl_shm::WlShm,
        _event: <wl_shm::WlShm as wayland_client::Proxy>::Event,
        _data: &(),
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
    ) {
    }
}

impl Dispatch<wl_output::WlOutput, ()> for State {
    fn event(
        _state: &mut Self,
        _proxy: &wl_output::WlOutput,
        _event: <wl_output::WlOutput as wayland_client::Proxy>::Event,
        _data: &(),
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
    ) {
    }
}

impl Dispatch<wl_shm_pool::WlShmPool, ()> for State {
    fn event(
        _state: &mut Self,
        _proxy: &wl_shm_pool::WlShmPool,
        _event: <wl_shm_pool::WlShmPool as wayland_client::Proxy>::Event,
        _data: &(),
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
    ) {
    }
}

impl Dispatch<wl_buffer::WlBuffer, ()> for State {
    fn event(
        _state: &mut Self,
        _proxy: &wl_buffer::WlBuffer,
        _event: <wl_buffer::WlBuffer as wayland_client::Proxy>::Event,
        _data: &(),
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
    ) {
    }
}

impl Dispatch<zwlr_screencopy_frame_v1::ZwlrScreencopyFrameV1, ()> for State {
    fn event(
        state: &mut Self,
        _frame: &zwlr_screencopy_frame_v1::ZwlrScreencopyFrameV1,
        event: zwlr_screencopy_frame_v1::Event,
        _data: &(),
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
    ) {
        if state.debug {
            info!("Received frame event: {:?}", event);
        }

        match event {
            zwlr_screencopy_frame_v1::Event::Buffer { format, width, height, stride } => {
                if state.debug {
                    info!("Frame buffer event received:");
                    info!("- Format: {:?}", format);
                    info!("- Dimensions: {}x{}", width, height);
                    info!("- Stride: {}", stride);
                }
                state.frame_state.width = width;
                state.frame_state.height = height;
                state.frame_state.stride = stride;
            }
            zwlr_screencopy_frame_v1::Event::BufferDone => {
                if state.debug {
                    info!("Frame buffer done event received");
                }
                state.frame_state.buffer_done = true;
            }
            zwlr_screencopy_frame_v1::Event::Ready { tv_sec_hi, tv_sec_lo, tv_nsec } => {
                if state.debug {
                    info!("Frame ready event received:");
                    info!("- Time: {}.{:09} seconds", ((tv_sec_hi as u64) << 32) + tv_sec_lo as u64, tv_nsec);
                }
                state.frame_state.done = true;
            }
            zwlr_screencopy_frame_v1::Event::Failed => {
                if state.debug {
                    info!("Frame failed event received");
                }
                state.frame_state.failed = true;
            }
            _ => {
                if state.debug {
                    info!("Unhandled frame event: {:?}", event);
                }
            }
        }
    }
} 