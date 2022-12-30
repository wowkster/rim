use std::io::{Result, Write};

use anes::execute;
use anes::ClearBuffer;
use anes::Color;
use anes::MoveCursorDown;
use anes::MoveCursorLeft;
use anes::MoveCursorRight;
use anes::MoveCursorTo;
use anes::MoveCursorToNextLine;
use anes::MoveCursorToPreviousLine;
use anes::MoveCursorUp;
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

enum EditorMode {
    Normal,
    Insert,
}

struct Editor {
    width: usize,
    height: usize,
    text_buffer: String,
    cursor_index: usize,
    mode: EditorMode,
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
            width: 0,
            height: 0,
            text_buffer: String::from(
                "\
                struct Editor {\n\
                \x20\x20\x20\x20w: usize,\n\
                \x20\x20\x20\x20h: usize,\n\
                \x20\x20\x20\x20text_buffer: String,\n\
                \x20\x20\x20\x20cursor_index: usize,\n\
                }\n\
                \n\
                fn main() {\n\
                \x20\x20\x20\x20println!(\"Hello World\");\n\
                }",
            ),
            cursor_index: 0,
            mode: EditorMode::Normal,
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

        loop {
            let resized = self.resize_if_changed();

            if should_render || resized {
                self.render().expect("Failed to render screen");
            }

            should_render = true;

            if let KeyEvent(key) = WinConsole::input().read_single_input().unwrap() {
                // Only check for key down events
                if key.key_down {
                    let char_value = key.u_char;
                    // Write only if is alphanumeric or punctuation
                    if char_value.is_ascii_alphanumeric() || char_value.is_ascii_punctuation() {
                        match self.mode {
                            EditorMode::Normal => self.handle_normal_char(char_value),
                            EditorMode::Insert => self.handle_insert_char(char_value),
                        }
                    } else {
                        match key.virtual_key_code {
                            ESCAPE => {
                                return;
                            }
                            ENTER => self.move_cursor_to_next_line(),
                            SPACE => self.move_cursor_right(),
                            BACKSPACE => self.move_cursor_left(),
                            ARROW_RIGHT => self.move_cursor_right(),
                            ARROW_LEFT => self.move_cursor_left(),
                            ARROW_DOWN => self.move_cursor_down(),
                            ARROW_UP => self.move_cursor_up(),
                            code => {
                                todo!("Handle key code: {code} (0x{code:x?})");
                            }
                        }
                    }
                }
            }
        }
    }

    fn get_content_of_row(&self, row: usize) -> Option<&str> {
        let mut lines = self.text_buffer.lines();

        lines.nth(row)
    }

    fn get_num_rows(&self) -> usize {
        let lines: Vec<_> = self.text_buffer.lines().collect();

        lines.len()
    }

    fn get_cursor_row_index(&self) -> usize {
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

    fn get_cursor_col_index(&self) -> usize {
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

    fn move_cursor_right(&mut self) {
        let mut stdout = std::io::stdout();

        // If at end of file, don't move the cursor
        if self.cursor_index == self.text_buffer.len() {
            return;
        }

        /* Get the current cursor row and column */
        let row = self.get_cursor_row_index();
        let row = self
            .get_content_of_row(row)
            .expect("Cursor row was not in bounds of text_buffer");
        let row_len = row.len();
        let col = self.get_cursor_col_index();

        // Increment the cursor index
        self.cursor_index += 1;

        if col < row_len {
            /* Cursor is not at the end of a line */
            execute!(&mut stdout, MoveCursorRight(1)).expect("Could not move cursor right");
        } else {
            /* Cursor is at the end of a line */
            execute!(&mut stdout, MoveCursorToNextLine(1))
                .expect("Could not move cursor to next line");
        }
    }

    fn move_cursor_left(&mut self) {
        let mut stdout = std::io::stdout();

        // If at beginning of file, don't move the cursor
        if self.cursor_index == 0 {
            return;
        }

        /* Get the current cursor row and column */
        let col = self.get_cursor_col_index();

        // Increment the cursor index
        self.cursor_index -= 1;

        if col > 0 {
            /* Cursor is not at the end of a line */
            execute!(&mut stdout, MoveCursorLeft(1)).expect("Could not move cursor left");
        } else {
            /* Cursor is at the end of a line */
            let current_row_index = self.get_cursor_row_index();
            let previous_row = self
                .get_content_of_row(current_row_index)
                .expect("Tried to move cursor up when no line was found above");

            execute!(&mut stdout, MoveCursorToPreviousLine(1))
                .expect("Could not move cursor to previous line");

            if previous_row.len() > 0 {
                execute!(&mut stdout, MoveCursorRight(previous_row.len() as u16))
                    .expect("Could not move cursor to end of previous line");
            }
        }
    }

    fn move_cursor_down(&mut self) {
        let mut stdout = std::io::stdout();

        let row_index = self.get_cursor_row_index();

        // If at end of file, don't move the cursor
        if self.get_num_rows() == row_index + 1 {
            return;
        }

        let col_index = self.get_cursor_col_index();

        let current_row = self
            .get_content_of_row(row_index)
            .expect("Could not get content of current row");
        let next_row = self
            .get_content_of_row(row_index + 1)
            .expect("Could not get content of next row");
        let next_row_len = next_row.len();

        if next_row.len() < col_index + 1 {
            /* Go to end next line */

            // Move cursor index by ((what is left of the current line) + \n + (text content of next line up until the cursor col))
            self.cursor_index += &current_row[col_index..].len() + 1;
            self.cursor_index += next_row_len;

            execute!(&mut stdout, MoveCursorToNextLine(1),)
                .expect("Could not move cursor to next line");

            if next_row_len > 0 {
                execute!(&mut stdout, MoveCursorRight(next_row_len as u16),)
                    .expect("Could not move cursor to end of next line");
            }
        } else {
            /* Move cursor down one space */

            // Move cursor index by ((what is left of the current line) + \n + (text content of next line up until the cursor col))
            self.cursor_index += &current_row[col_index..].len() + 1;
            self.cursor_index += col_index;

            execute!(&mut stdout, MoveCursorDown(1))
                .expect("Could not move cursor to previous line");
        }
    }

    fn move_cursor_up(&mut self) {
        let mut stdout = std::io::stdout();

        let row_index = self.get_cursor_row_index();

        // If at end of file, don't move the cursor
        if row_index == 0 {
            return;
        }

        let col_index = self.get_cursor_col_index();

        let current_row = self
            .get_content_of_row(row_index)
            .expect("Could not get content of current row");
        let previous_row = self
            .get_content_of_row(row_index - 1)
            .expect("Could not get content of previous row");
        let previous_row_len = previous_row.len();

        if previous_row.len() < col_index + 1 {
            /* Go to end previous line */

            // Move cursor index by ((what is left of the current line) + \n + (text content of previous line up until the cursor col))
            self.cursor_index -= &current_row[..col_index].len() + 1;

            execute!(&mut stdout, MoveCursorToPreviousLine(1))
                .expect("Could not move cursor to previous line");

            if previous_row_len > 0 {
                execute!(&mut stdout, MoveCursorRight(previous_row_len as u16))
                    .expect("Could not move cursor to end of previous line");
            }
        } else {
            /* Move cursor up one space */

            // Move cursor index by ((what is left of the current line) + \n + (text content of next line up until the cursor col))
            self.cursor_index -= &previous_row[col_index..].len() + 1;
            self.cursor_index -= col_index;

            execute!(&mut stdout, MoveCursorUp(1)).expect("Could not move cursor to previous line");
        }
    }

    fn move_cursor_to_next_line(&mut self) {
        let mut stdout = std::io::stdout();

        let row_index = self.get_cursor_row_index();

        // If at end of file, don't move the cursor
        if self.get_num_rows() == row_index + 1 {
            return;
        }

        let col_index = self.get_cursor_col_index();

        let current_row = self
            .get_content_of_row(row_index)
            .expect("Could not get content of current row");

        // Move cursor index by (what is left of the current line) + \n
        self.cursor_index += &current_row[col_index..].len() + 1;

        execute!(&mut stdout, MoveCursorToNextLine(1),)
            .expect("Could not move cursor to next line");
    }

    /**
     * Handle movement inputs in normal mode
     */
    fn handle_normal_char(&mut self, char_value: char) {
        match char_value {
            'i' => self.mode = EditorMode::Insert,
            _ => todo!(
                "Handle ascii text char: {char_value} (0x{:x?}) in NORMAL mode",
                char_value as u32
            ),
        }
    }

    /**
     * Handle text input in insert mode
     */
    fn handle_insert_char(&mut self, char_value: char) {
        assert!(
            char_value.is_ascii_alphanumeric() || char_value.is_ascii_punctuation(),
            "Character is not alphanumeric"
        );

        let current_row_index = self.get_cursor_row_index();
        let current_row_content = self
            .get_content_of_row(current_row_index)
            .expect("Could not get content of current row");

        if current_row_content.len() >= self.width {
            todo!("Handle inserting on line longer than screen width")
        }

        self.text_buffer.insert(self.cursor_index, char_value);

        self.move_cursor_right();
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

        for row in 0..self.height - 1 {
            execute!(&mut render_buffer, SetForegroundColor(Color::Default))?;

            let line = lines.get(row as usize);

            if let Some(line) = line {
                // Print line

                let slice = if line.len() < self.width {
                    &line[0..]
                } else {
                    &line[0..self.width]
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

        let row_index = self.get_cursor_row_index();
        let row_text = self
            .get_content_of_row(row_index)
            .expect("Cursor row was not in bounds of text_buffer");

        let row_len = row_text.len();

        let col_index = self.get_cursor_col_index();

        write!(
            &mut render_buffer,
            "{} | Cursor Index: {} | Row Index: {} | Col Index: {} | Row Length: {}",
            match self.mode {
                EditorMode::Normal => "-- NORMAL --",
                EditorMode::Insert => "-- INSERT --",
            },
            self.cursor_index, row_index, col_index, row_len
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
        if w == self.width && h == self.height {
            return false;
        }

        // Clear the screen buffer if the size changed
        let mut stdout = std::io::stdout();
        execute!(&mut stdout, ClearBuffer::All).expect("Could not clear terminal buffer on resize");

        // Set the new size for next render
        self.width = w;
        self.height = h;

        true
    }
}
