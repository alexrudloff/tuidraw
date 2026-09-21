"""Project workflow smoke check. After cargo build: python3 tests/projects.py. No model calls."""
import json
import os
import re
import unicodedata
from pathlib import Path
import tempfile
from builder import BIN, ROOT, command, terminal


def screen(output):
    """Replay the cursor/erase subset emitted by our Crossterm backend for UI assertions."""
    cells = [[' '] * 100 for _ in range(36)]
    x = y = 0
    for match in re.finditer(r'\x1b\[([0-9;?]*)([A-Za-z])|([^\x1b]+)', output.decode(errors='replace')):
        params, code, text = match.groups()
        if code:
            values = [int(v or 0) for v in params.split(';')] if not params.startswith('?') else []
            n = (values[0] if values else 0) or 1
            if code in ('H', 'f'):
                y, x = n - 1, ((values[1] if len(values) > 1 else 0) or 1) - 1
            elif code == 'G': x = n - 1
            elif code == 'A': y -= n
            elif code == 'B': y += n
            elif code == 'C': x += n
            elif code == 'D': x -= n
            elif code == 'J' and values == [2]: cells = [[' '] * 100 for _ in range(36)]
            elif code == 'K' and 0 <= y < 36:
                for i in range(max(0, x), 100): cells[y][i] = ' '
        else:
            for char in text:
                if char == '\r': x = 0
                elif char == '\n': y += 1
                elif not char.isprintable(): continue
                elif not unicodedata.combining(char):
                    if 0 <= y < 36 and 0 <= x < 100: cells[y][x] = char
                    x += 2 if unicodedata.east_asian_width(char) in ('W', 'F') else 1
    return '\n'.join(''.join(row).rstrip() for row in cells)


def read(project):
    return json.loads((project / '.builder/session.json').read_text())


def main():
    with tempfile.TemporaryDirectory(prefix='tui-projects-') as folder:
        base = Path(folder).resolve()
        env = {**os.environ, 'XDG_STATE_HOME': str(base / 'state')}
        alpha, beta, new = base / 'alpha', base / 'beta', base / 'new game'
        command('build', '--spec', ROOT / 'examples/primitives.json', '--out', alpha, env=env)
        source = json.loads((ROOT / 'examples/primitives.json').read_text())
        source['elements']['heading']['props']['text'] = 'BETA PROJECT'
        (base / 'beta.json').write_text(json.dumps(source))
        command('build', '--spec', base / 'beta.json', '--out', beta, env=env)
        (alpha / 'custom.txt').write_text('Keep all user files')
        (base / 'alpha-link').symlink_to(alpha, target_is_directory=True)

        def controls(fd, wait, process):
            def send(data): os.write(fd, data)
            def visible(text):
                def ready(out):
                    view = screen(out)
                    return text in view and (text != 'TUI Draw · Projects' or 'Ctrl+G Chat' not in view) and (text != 'keep-my-draft' or 'Ctrl+G Chat' in view)
                wait(ready)
            visible('TUI Draw · Projects')
            wait(lambda out: len(re.findall(r'^\s*│\s+[› ] alpha\s*│$', screen(out), re.M)) == 1)
            assert not (base / 'tui-project').exists()
            send(b'\t')
            visible('Enter open project')
            send(b'\t')
            visible('Enter delete')
            send(b'\x1b[Z\x1b[Z')  # BackTab twice returns to the list.
            visible('Enter open ·')
            send(b'\x1b[B\r')
            visible('Ctrl+G Chat')
            send(b'\x07keep-my-draft')
            visible('keep-my-draft')
            send(b'\x1b')
            wait(lambda out: 'keep-my-draft' not in screen(out))
            send(b'7\x13')  # Closing chat focuses the first input: count 2 -> 27.
            wait(lambda _: len(read(alpha)['turns']) == 2)
            assert read(alpha)['turns'][-1]['spec']['state']['count'] == '27'
            send(b'\x07')
            visible('keep-my-draft')
            send(b'\x10')  # Project menu saves before switching; Esc retains the draft.
            visible('TUI Draw · Projects')
            send(b'\x1b')
            visible('keep-my-draft')
            send(b'\x0ealpha\r')
            visible('not empty')
            send(b'\x1b')
            visible('keep-my-draft')
            send(b'\x0fmissing-project\r')
            visible('No saved project')
            assert not (base / 'missing-project').exists()
            send(b'\x1b')
            visible('keep-my-draft')
            send(b'\x0fbeta\r')
            visible('BETA PROJECT')
            # Delete uses the current selection and defaults to Cancel, never a name field.
            send(b'\x10\x1b[B\x04')
            visible('Delete alpha?')
            wait(lambda out: 'Cancel' in screen(out) and 'Project folder' not in screen(out))
            send(b'\r')  # Enter on the default Cancel.
            visible('TUI Draw · Projects')
            assert alpha.exists()
            send(b'\x04')
            visible('Delete alpha?')
            send(b'\x1by')
            wait(lambda _: not alpha.exists())
            trashed = list((base / '.tui-draw-trash').glob('alpha-*'))
            assert len(trashed) == 1 and (trashed[0] / 'custom.txt').read_text() == 'Keep all user files'
            assert read(trashed[0])['turns'][-1]['spec']['state']['count'] == '27'
            assert not (trashed[0] / '.builder/write.lock').exists()
            visible('Project deleted.')
            visible('Undo delete')
            # Undo never replaces a new folder at the original path, and remains retryable.
            alpha.mkdir()
            (alpha / 'replacement.txt').write_text('Do not replace me')
            send(b'\x1bu')
            visible('Cannot restore:')
            visible('Enter undo delete')
            assert (alpha / 'replacement.txt').read_text() == 'Do not replace me'
            assert (trashed[0] / 'custom.txt').exists()
            (alpha / 'replacement.txt').unlink()
            alpha.rmdir()
            send(b'\r')
            visible('Project restored.')
            assert (alpha / 'custom.txt').read_text() == 'Keep all user files'
            assert read(alpha)['turns'][-1]['spec']['state']['count'] == '27'
            assert not (alpha / '.builder/write.lock').exists()
            assert not trashed[0].exists()
            send(b'\x04')
            visible('Delete alpha?')
            send(b'\t\r')
            wait(lambda _: not alpha.exists())
            # A live writer blocks deletion; deleting the active project cannot resurrect it.
            send(b'\x04')
            visible('Delete beta?')
            lock = beta / '.builder/write.lock'
            lock.write_text('')
            send(b'\t\r')
            visible('being written')
            assert beta.exists()
            lock.unlink()
            send(b'\r')
            wait(lambda _: not beta.exists())
            send(b'\rnew game\r')
            wait(lambda _: (new / '.builder/session.json').is_file())
            visible('Describe an interface')
            assert not read(new)['turns']
            assert process.poll() is None
        terminal([str(BIN)], env, controls, cwd=base)
        assert not alpha.exists() and not beta.exists()
        assert not (base / 'tui-project').exists()

        def resume(fd, wait, _):
            wait(lambda out: 'new game' in screen(out))
            # Open existing empty project through the saved project list.
            os.write(fd, b'\r')
            wait(lambda out: 'Describe an interface' in screen(out))
        terminal([str(BIN)], env, resume, cwd=base)
        def mouse_controls(fd, wait, _):
            def click(text, last=False):
                location = []
                def find(out):
                    hits = [(line.index(text), y) for y, line in enumerate(screen(out).splitlines()) if text in line]
                    if hits:
                        location[:] = [hits[-1] if last else hits[0]]
                        return True
                    return False
                wait(find)
                x,y = location[0]
                os.write(fd, f'\x1b[<0;{x+2};{y+1}M\x1b[<0;{x+2};{y+1}m'.encode())
            click('New project')
            wait(lambda out: 'Create project' in screen(out))
            os.write(fd,b'mouse-made')
            click('Create project')
            wait(lambda _: (base / 'mouse-made/.builder/session.json').exists())
            os.write(fd,b'\x10')
            wait(lambda out: 'TUI Draw · Projects' in screen(out) and 'Ctrl+G Chat' not in screen(out))
            click('Delete')
            wait(lambda out: 'Delete mouse-made?' in screen(out))
            click('Cancel')
            wait(lambda out: 'TUI Draw · Projects' in screen(out))
            assert (base / 'mouse-made').exists()
            click('Delete')
            wait(lambda out: 'Delete mouse-made?' in screen(out))
            click('Delete project', last=True)
            wait(lambda _: not (base / 'mouse-made').exists())
            click('Open')
            wait(lambda out: 'Describe an interface' in screen(out))
        terminal([str(BIN)],env,mouse_controls,cwd=base)

        def accelerators(fd, wait, _):
            def visible(text): wait(lambda out:text in screen(out))
            visible('Alt+N')
            visible('Alt+A')
            visible('Alt+O')
            visible('Alt+D')
            os.write(fd,b'\x1bn')
            visible('Create project')
            os.write(fd,b'accelerator-made\x1bn')
            wait(lambda _: (base/'accelerator-made/.builder/session.json').exists())
            visible('Describe an interface')
            os.write(fd,b'\x10')
            visible('TUI Draw · Projects')
            os.write(fd,b'\x1bd')
            visible('Delete accelerator-made?')
            os.write(fd,b'\x1bc')
            visible('TUI Draw · Projects')
            assert (base/'accelerator-made').exists()
            os.write(fd,b'\x1bd')
            visible('Delete accelerator-made?')
            os.write(fd,b'\x1by')
            wait(lambda _: not (base/'accelerator-made').exists())
            visible('Alt+U')
            os.write(fd,b'\x1ba')
            visible('Add existing project')
            os.write(fd,b'new game\x1bo')
            visible('Describe an interface')
            os.write(fd,b'\x10')
            visible('TUI Draw · Projects')
            os.write(fd,b'\x1bo')
            visible('Describe an interface')
        terminal([str(BIN)],env,accelerators,cwd=base)

        # Many projects: after End scrolls the Select, clicking a visible row must select that row.
        for index in range(12):
            project = base / f'project-{index:02}' / '.builder'
            project.mkdir(parents=True)
            (project / 'session.json').write_text(json.dumps(read(new)))
        def scrolled_list(fd, wait, _):
            wait(lambda out: 'TUI Draw · Projects' in screen(out))
            os.write(fd,b'\x1b[F')
            chosen = []
            def find_row(out):
                view=screen(out)
                if not re.search(r'› project-\d\d', view): return False
                for y,line in enumerate(view.splitlines()):
                    match=re.search(r'project-\d\d',line)
                    if match:
                        chosen[:] = [match.group(),match.start(),y]
                        return True
                return False
            wait(find_row)
            name,x,y=chosen
            os.write(fd,f'\x1b[<0;{x+1};{y+1}M\x1b[<0;{x+1};{y+1}m\r'.encode())
            wait(lambda out: f'TUI Draw · {name}' in screen(out))
        terminal([str(BIN)],env,scrolled_list,cwd=base)
        print('PASS: centered launcher, create/open/delete/recover, mouse buttons, scrolled list hit-testing, lock protection, draft toggle, preview save, switch/cancel, empty-project resume')


if __name__ == '__main__':
    main()
