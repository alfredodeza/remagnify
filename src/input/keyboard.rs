use wayland_client::protocol::wl_keyboard::WlKeyboard;
use xkbcommon::xkb;

pub struct Keyboard {
    pub keyboard: WlKeyboard,
    pub xkb_context: xkb::Context,
    pub xkb_state: Option<xkb::State>,
}

impl Keyboard {
    pub fn new(keyboard: WlKeyboard) -> anyhow::Result<Self> {
        let xkb_context = xkb::Context::new(xkb::CONTEXT_NO_FLAGS);

        Ok(Self {
            keyboard,
            xkb_context,
            xkb_state: None,
        })
    }

    pub fn handle_keymap(&mut self, format: u32, fd: i32, _size: u32) -> anyhow::Result<()> {
        use std::os::unix::io::FromRawFd;

        if format != 1 {
            // XKB_KEYMAP_FORMAT_TEXT_V1
            log::warn!("Unsupported keymap format: {}", format);
            return Ok(());
        }

        unsafe {
            let mut file = std::fs::File::from_raw_fd(fd);
            let keymap = xkb::Keymap::new_from_file(
                &self.xkb_context,
                &mut file,
                xkb::KEYMAP_FORMAT_TEXT_V1,
                xkb::KEYMAP_COMPILE_NO_FLAGS,
            )
            .ok_or_else(|| anyhow::anyhow!("Failed to create XKB keymap"))?;

            self.xkb_state = Some(xkb::State::new(&keymap));
        }

        Ok(())
    }

    pub fn handle_key(&self, key: u32, state: u32) -> Option<xkb::Keysym> {
        if state == 0 {
            // Released
            return None;
        }

        self.xkb_state.as_ref().map(|xkb_state| xkb_state.key_get_one_sym((key + 8).into()))
    }
}
