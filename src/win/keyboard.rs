use std::collections::HashSet;
use std::fmt::Formatter;
use std::{ptr, thread};
use winapi::um::winuser::{CallNextHookEx, INPUT, INPUT_KEYBOARD, KBDLLHOOKSTRUCT, keybd_event, KEYBDINPUT, KEYEVENTF_KEYUP, KEYEVENTF_SCANCODE, KEYEVENTF_UNICODE, MapVirtualKeyW, SendInput};
use crate::keymanager::KEY_MANAGER_INSTANCE;
use crate::win::{HC_ACTION, KEYBOARD_HOOK, MessageType, ToChar, ToUnicode, VIRTUAL_KEY};
use num_traits::FromPrimitive;
use std::fmt::Debug;
use std::time::Duration;
use winapi::shared::minwindef::{BYTE, UINT};
use crate::win::keyboard_vk::KNOWN_VIRTUAL_KEY;
use crate::win::keyboard_vk::KNOWN_VIRTUAL_KEY::{VK_LMENU, VK_LSHIFT, VK_LWIN, VK_MENU, VK_PACKET, VK_RMENU, VK_RSHIFT, VK_RWIN, VK_SHIFT};

struct KBDStructWrapper(KBDLLHOOKSTRUCT);

impl Debug for KBDStructWrapper {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "KBDLLHOOKSTRUCT vk: {} | {:?}, scan: {}, flags: {}, time: {}, extra: {} | char [{}|{}] | unicode [{:?}|{:?}]", self.0.vkCode, KNOWN_VIRTUAL_KEY::try_from(self.0.vkCode), self.0.scanCode, self.0.flags, self.0.time, self.0.dwExtraInfo, self.0.vkCode.to_char(), self.0.vkCode.to_char_localized(), self.0.vkCode.to_unicode(), self.0.vkCode.to_unicode_localized())
    }
}


pub extern "system" fn keyboard_hook_proc(n_code: i32, w_param: usize, l_param: isize) -> isize {
    let handled = if n_code == HC_ACTION {
        if let Some(ev) = KEYBOARD_HOOK::from_u32(w_param as u32) {
            let kbd_struct = unsafe { *(l_param as *const KBDLLHOOKSTRUCT) };
            // VK_PACKET is sent when someone sends unicode characters
            if kbd_struct.vkCode == VK_PACKET as u32 {
                None
            } else {
                match ev {
                    KEYBOARD_HOOK::WM_KEYDOWN | KEYBOARD_HOOK::WM_SYSKEYDOWN => {
                        println!("press: {:?}", KBDStructWrapper(kbd_struct));
                        let result = &KEY_MANAGER_INSTANCE.write().keydown(kbd_struct.vkCode as _);
                        if *result == true {
                            //println!("IT FUKKEN WORKED?");
                            Some(1)
                        } else {
                            None
                        }
                    }
                    KEYBOARD_HOOK::WM_KEYUP | KEYBOARD_HOOK::WM_SYSKEYUP => {
                        let result = &KEY_MANAGER_INSTANCE.write().keyup(kbd_struct.vkCode as _);

                        None
                    }
                    //_ => {}
                }
            }
            //None//Some(1)
        } else {
            None
        }
    } else {
        None
    };

    return match handled {
        None => {
            println!("[KBD] aNo one handled message, redirecting to the next hook :(");
            unsafe { CallNextHookEx(ptr::null_mut(), n_code, w_param, l_param) }
        }
        Some(res) => {
            res
        }
    };
}

pub fn release_virtual_keys(keys: HashSet<VIRTUAL_KEY>) {
    for vk in keys {
        let scancode = unsafe { MapVirtualKeyW(vk as UINT, 0) as BYTE }; // Get the scan code for the virtual key

        unsafe {
            // Release the key
            keybd_event(vk as BYTE, scancode, KEYEVENTF_KEYUP, 0);
        }
    }
}
pub fn press_virtual_keys(keys: HashSet<VIRTUAL_KEY>) {
    for vk in keys {
        let scancode = unsafe { MapVirtualKeyW(vk as UINT, 0) as BYTE }; // Get the scan code for the virtual key

        unsafe {
            // Release the key
            keybd_event(vk as BYTE, scancode, 0, 0);
        }
    }
}

pub fn send_unicode_character(ch: char) {
    let mut inputs = [
        INPUT {
            type_: INPUT_KEYBOARD,
            u: unsafe { std::mem::zeroed() },
        },
        INPUT {
            type_: INPUT_KEYBOARD,
            u: unsafe { std::mem::zeroed() },
        },
    ];

    let ki_press = KEYBDINPUT {
        wVk: 0,
        wScan: ch as u16,
        dwFlags: KEYEVENTF_UNICODE,
        time: 0,
        dwExtraInfo: 0,
    };

    let ki_release = KEYBDINPUT {
        wVk: 0,
        wScan: ch as u16,
        dwFlags: KEYEVENTF_UNICODE | KEYEVENTF_KEYUP,
        time: 0,
        dwExtraInfo: 0,
    };
    println!("Firing {:x}", ch as u16);

    unsafe {
        *inputs[0].u.ki_mut() = ki_press; // Press event
        *inputs[1].u.ki_mut() = ki_release; // Release event
        SendInput(inputs.len() as UINT, inputs.as_mut_ptr(), std::mem::size_of::<INPUT>() as i32);
    }
}

pub fn filter_modifier_keys(vk_list: &HashSet<VIRTUAL_KEY>) -> Vec<VIRTUAL_KEY> {
    // Define a list of modifier keys
    let modifiers: Vec<VIRTUAL_KEY> = vec![
        VK_SHIFT as u32, // VK_SHIFT
        VK_LSHIFT as u32, // VK_LSHIFT
        VK_RSHIFT as u32, // VK_RSHIFT
        VK_MENU as u32, // VK_MENU (Alt)
        VK_LMENU as u32, // VK_LMENU (LAlt)
        VK_RMENU as u32, // VK_RMENU (RAlt)
        VK_LWIN as u32, // VK_LWIN
        VK_RWIN as u32, // VK_RWIN
    ];

    // Filter the input list to include only the modifier keys
    vk_list.iter()
        .cloned()
        .filter(|vk| modifiers.contains(vk))
        .collect()
}
// Function to send a sequence of keypresses, a Unicode character, and another sequence of keypresses
#[derive(Clone, Copy)]
pub(crate) struct KeyStroke {
    pub(crate) key_type: KeyType,
    pub(crate) scancode: u32, // Use for ScanCode and Unicode as character code
    pub(crate) action: KeyAction,
}

#[derive(Clone, Copy)]
pub enum KeyType {
    Unicode,
    ScanCode,
}

#[derive(Clone, Copy, PartialEq)]
pub enum KeyAction {
    Press,
    Release,
}

pub(crate) fn send_key_sequence(pre_keys: &[KeyStroke], ch: char, post_keys: &[KeyStroke]) {
    let mut inputs = Vec::new();

    // Add pre-keypress actions
    pre_keys.iter().for_each(|&key| inputs.push(create_input(key)));

    // Add the Unicode character input and its release
    inputs.push(create_input(KeyStroke {
        key_type: KeyType::Unicode,
        scancode: ch as u32,
        action: KeyAction::Press,
    }));
    thread::sleep(Duration::from_millis(100));
    unsafe {
        SendInput(inputs.len() as UINT, inputs.as_mut_ptr(), std::mem::size_of::<INPUT>() as i32);
    }
    //thread::sleep(Duration::from_millis(100));
    let mut inputs = Vec::new();
    inputs.push(create_input(KeyStroke {
        key_type: KeyType::Unicode,
        scancode: ch as u32,
        action: KeyAction::Release,
    }));

    // Add post-keypress actions
    post_keys.iter().for_each(|&key| inputs.push(create_input(key)));

    // Send the input sequence
    unsafe {
        SendInput(inputs.len() as UINT, inputs.as_mut_ptr(), std::mem::size_of::<INPUT>() as i32);
    }
}

fn create_input(stroke: KeyStroke) -> INPUT {
    let mut input = INPUT {
        type_: INPUT_KEYBOARD,
        u: unsafe { std::mem::zeroed() },
    };

    unsafe {
        let ki = match stroke.key_type {
            KeyType::Unicode => {
                KEYBDINPUT {
                    wVk: 0, // Virtual-key code is not needed for Unicode input
                    wScan: stroke.scancode as u16, // Unicode character code
                    dwFlags: KEYEVENTF_UNICODE | if stroke.action == KeyAction::Release { KEYEVENTF_KEYUP } else { 0 },
                    time: 0,
                    dwExtraInfo: 0,
                }
            },
            KeyType::ScanCode => {
                KEYBDINPUT {
                    wVk: 0, // Virtual-key code is not needed for scancode input
                    wScan: stroke.scancode as u16, // Scancode
                    dwFlags: KEYEVENTF_SCANCODE | if stroke.action == KeyAction::Release { KEYEVENTF_KEYUP } else { 0 },
                    time: 0,
                    dwExtraInfo: 0,
                }
            },
        };

        *input.u.ki_mut() = ki;
    }

    input
}
