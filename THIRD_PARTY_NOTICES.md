# tuiparts recipe adaptations

The native button, badge and switch presentation and semantic theme roles in `src/controls.rs` and `src/lib.rs` are adapted from tuiparts, by Matt Simpson and the tuiparts contributors. Reference revision: `73b3622d4b989e4e4102f3ae85b364bdda7f76e2`.

Sources: https://github.com/tuiparts/tuiparts/tree/73b3622d4b989e4e4102f3ae85b364bdda7f76e2/registry (button, badge, switch, theme recipes).

The upstream README and package metadata declare the MIT license. No OpenTUI runtime is embedded. Keypad composition and arithmetic are original application code.

Copyright tuiparts contributors.

Permission is hereby granted, free of charge, to any person obtaining a copy of this software and associated documentation files (the "Software"), to deal in the Software without restriction, including without limitation the rights to use, copy, modify, merge, publish, distribute, sublicense, and/or sell copies of the Software, and to permit persons to whom the Software is furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in all copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY, FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM, OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE SOFTWARE.

## Native widget dependencies

The application links the published `tui-widgets` crates from https://github.com/ratatui/tui-widgets (MIT OR Apache-2.0) and `ansi-to-tui` from https://github.com/ratatui/ansi-to-tui (MIT). Their sources and license files ship through Cargo dependencies; no upstream source is vendored here. `qrcode` is MIT OR Apache-2.0. See Cargo.lock for exact versions.
