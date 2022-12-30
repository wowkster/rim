use std::io::{Result, Write};

use anes::execute;
use anes::ClearBuffer;
use anes::Color;
use anes::MoveCursorLeft;
use anes::MoveCursorRight;
use anes::MoveCursorTo;
use anes::MoveCursorToNextLine;
use anes::MoveCursorToPreviousLine;
use anes::RestoreCursorPosition;
use anes::SaveCursorPosition;
use anes::SetForegroundColor;
use anes::SwitchBufferToAlternate;
use anes::SwitchBufferToNormal;
use win32console::console::WinConsole;
use win32console::input::InputRecord::KeyEvent;
use winapi::shared::minwindef::BOOL;
use winapi::shared::minwindef::DWORD;
use winapi::um::consoleapi::SetConsoleCtrlHandler;
use winapi::um::wincon::CTRL_C_EVENT;

fn main() {
    Editor::start();
}

struct Editor {
    w: usize,
    h: usize,
    text_buffer: String,
    cursor_index: usize,
}

impl Editor {
    /**
     * Cleanup for the editor when the program exits
     *
     * Can be called if:
     *  - The program exits normally
     *  - A Ctrl signal is sent to the program by Windows
     *  - The program panics
     */
    fn cleanup() {
        let mut stdout = std::io::stdout();

        execute!(&mut stdout, SwitchBufferToNormal).expect("Could not switch back terminal buffer");
        execute!(&mut stdout, SetForegroundColor(Color::Default))
            .expect("Could not switch back terminal color");
    }

    fn start() {
        let editor = Editor {
            w: 0,
            h: 0,
            text_buffer: String::from(
                "fn main() {\n\
                \x20\x20\x20\x20println!(\"Hello World\");\n\
                }",
            ),
            cursor_index: 0,
        };

        /*
         * Cleanup the editor if the program panics
         */
        std::panic::set_hook(Box::new(|info| {
            Editor::cleanup();
            eprintln!("{info}")
        }));

        /*
         * Cleanup the editor on a control signal, and then exit
         */
        unsafe {
            unsafe extern "system" fn control_handler(ctrl_type: DWORD) -> BOOL {
                Editor::cleanup();

                match ctrl_type {
                    CTRL_C_EVENT => println!("Got Ctrl+C"),
                    _ => eprintln!("Unknown Ctrl signal type"),
                }

                std::process::exit(0);
            }

            SetConsoleCtrlHandler(Some(control_handler), true as i32);
        }

        editor.run();

        /*
         * Cleanup the editor if the program exits normally
         */
        Editor::cleanup();
    }

    fn run(mut self) {
        let mut stdout = std::io::stdout();

        // Set up the terminal buffer
        execute!(&mut stdout, SwitchBufferToAlternate).expect("Could not switch terminal buffer");
        execute!(&mut stdout, ClearBuffer::All).expect("Could not clear terminal buffer");

        // Virtual key codes
        // https://docs.microsoft.com/en-us/windows/win32/inputdev/virtual-key-codes
        const ESCAPE: u16 = 0x1B;
        const BACKSPACE: u16 = 0x08;
        const ENTER: u16 = 0x0D;
        const SPACE: u16 = 0x20;
        const ARROW_LEFT: u16 = 0x25;
        const ARROW_UP: u16 = 0x26;
        const ARROW_RIGHT: u16 = 0x27;
        const ARROW_DOWN: u16 = 0x28;

        let mut should_render = true;

        'key_handler: loop {
            let resized = self.resize_if_changed();

            if should_render || resized {
                self.render().expect("Failed to render screen");
            }

            should_render = false;

            if let KeyEvent(key) = WinConsole::input().read_single_input().unwrap() {
                // Only check for key down events
                if key.key_down {
                    let char_value = key.u_char;
                    // Write only if is alphanumeric or punctuation
                    if char_value.is_ascii_alphanumeric() || char_value.is_ascii_punctuation() {
                        todo!(
                            "Handle ascii text char: {char_value} (0x{:x?})",
                            char_value as u32
                        );
                    } else {
                        match key.virtual_key_code {
                            ESCAPE => {
                                return;
                            }
                            ENTER => {
                                todo!("Insert new line at current cursor position")
                            }
                            SPACE => {
                                todo!("Insert space at current cursor position")
                            }
                            BACKSPACE => {
                                todo!("Remove char at current cursor position")
                            }
                            ARROW_RIGHT => {
                                // If at end of file, don't move the cursor
                                if self.cursor_index == self.text_buffer.len() {
                                    continue 'key_handler;
                                }

                                /* Get the current cursor row and column */
                                let row = self.get_cursor_row();
                                let row = self
                                    .get_text_row(row)
                                    .expect("Cursor row was not in bounds of text_buffer");
                                let row_len = row.len();
                                let col = self.get_cursor_col();

                                // Increment the cursor index
                                self.cursor_index += 1;

                                if col < row_len {
                                    /* Cursor is not at the end of a line */
                                    execute!(&mut stdout, MoveCursorRight(1))
                                        .expect("Could not move cursor right");
                                } else {
                                    /* Cursor is at the end of a line */
                                    execute!(&mut stdout, MoveCursorToNextLine(1))
                                        .expect("Could not move cursor to next line");
                                }
                            }
                            ARROW_LEFT => {
                                // If at beginning of file, don't move the cursor
                                if self.cursor_index == 0 {
                                    continue 'key_handler;
                                }

                                /* Get the current cursor row and column */
                                let col = self.get_cursor_col();

                                // Increment the cursor index
                                self.cursor_index -= 1;

                                if col > 0 {
                                    /* Cursor is not at the end of a line */
                                    execute!(&mut stdout, MoveCursorLeft(1))
                                        .expect("Could not move cursor left");
                                } else {
                                    /* Cursor is at the end of a line */
                                    let current_row_index = self.get_cursor_row();
                                    let previous_row = self.get_text_row(current_row_index).expect(
                                        "Tried to move cursor up when no line was found above",
                                    );

                                    execute!(
                                        &mut stdout,
                                        MoveCursorToPreviousLine(1),
                                        MoveCursorRight(previous_row.len() as u16)
                                    )
                                    .expect("Could not move cursor to previous line");
                                }
                            }
                            ARROW_DOWN => {
                                todo!("Handle arrow down")
                            }
                            ARROW_UP => {
                                todo!("Handle arrow up")
                            }
                            code => {
                                todo!("Handle key code: {code} (0x{code:x?})");
                            }
                        }
                    }
                }
            }
        }
    }

    fn get_text_row(&self, row: usize) -> Option<&str> {
        let mut lines = self.text_buffer.lines();

        lines.nth(row)
    }

    fn get_cursor_row(&self) -> usize {
        let mut row = 0;

        for (i, c) in self.text_buffer.chars().enumerate() {
            // If the index is between the start of this line and the end, return the current row number
            if self.cursor_index == i {
                return row;
            }

            if c == '\n' {
                row += 1;
            }
        }

        row
    }

    fn get_cursor_col(&self) -> usize {
        let mut chars = 0;

        for line in self.text_buffer.lines() {
            // If the index is between the start of this line and the end, the cursor's
            // column is the difference between the cursor index and the start of the line
            if self.cursor_index <= chars + line.len() {
                return self.cursor_index - chars;
            }

            chars += line.len() + 1;
        }

        0
    }

    fn render(&self) -> Result<()> {
        let mut stdout = std::io::stdout();

        execute!(
            &mut stdout,
            SaveCursorPosition,
            MoveCursorTo(0, 0),
            ClearBuffer::Below
        )?;

        let lines: Vec<_> = self.text_buffer.lines().collect();

        // Create a render buffer to limit write syscalls
        let mut render_buffer = Vec::new();

        for row in 0..self.h - 1 {
            execute!(&mut render_buffer, SetForegroundColor(Color::Default))?;

            let line = lines.get(row as usize);

            if let Some(line) = line {
                // Print line

                let slice = if line.len() < self.w {
                    &line[0..]
                } else {
                    &line[0..self.w]
                };

                write!(&mut render_buffer, "{}", slice)?;
            } else {
                // Print `~`

                execute!(&mut render_buffer, SetForegroundColor(Color::DarkBlue))?;
                write!(&mut render_buffer, "~")?;
            }

            write!(&mut render_buffer, "\r\n")?;

            execute!(&mut render_buffer, SetForegroundColor(Color::Default))?;
        }

        let row_index = self.get_cursor_row();
        let row_text = self
            .get_text_row(row_index)
            .expect("Cursor row was not in bounds of text_buffer");

        let row_len = row_text.len();

        let col_index = self.get_cursor_col();

        write!(
            &mut render_buffer,
            "Cursor Index: {} | Row Index: {} | Col Index: {} | Row Length: {} | Row Text : {:?}",
            self.cursor_index, row_index, col_index, row_len, row_text
        )?;

        execute!(&mut render_buffer, RestoreCursorPosition)?;

        // Flush render buffer to stdout in one write call
        stdout.write_all(&render_buffer)?;
        stdout.flush()?;

        Ok(())
    }

    fn resize_if_changed(&mut self) -> bool {
        let Some((w, h)) = term_size::dimensions() else {
            eprintln!("Unable to get term size :(");
            std::process::exit(1);
        };

        // Don't care unless size changed
        if w == self.w && h == self.h {
            return false;
        }

        // Clear the screen buffer if the size changed
        let mut stdout = std::io::stdout();
        execute!(&mut stdout, ClearBuffer::All).expect("Could not clear terminal buffer on resize");

        // Set the new size for next render
        self.w = w;
        self.h = h;

        true
    }
}
