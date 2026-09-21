"""After cargo build: CLI/chat/source-export smoke check, fake local model only."""
import fcntl
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
import json
import os
from pathlib import Path
import pty
import select
import struct
import subprocess
import tempfile
import termios
import threading
import time

ROOT = Path(__file__).resolve().parents[1]
BIN = ROOT / 'target/debug/tui-draw'
requests = []


class Model(BaseHTTPRequestHandler):
    def log_message(self, *_):
        pass

    def do_POST(self):
        body = json.loads(self.rfile.read(int(self.headers['Content-Length'])))
        messages = body['messages']
        request = next(m['content'] for m in reversed(messages) if m['role'] == 'user')
        requests.append(body)
        since_user = messages[max(i for i, m in enumerate(messages) if m['role'] == 'user') + 1:]
        draft = dict(updates=[dict(id='heading', key='text', value=request)])
        if request == 'invalid edit':
            draft['updates'][0]['key'] = 'unsupportedProp'
        if request == 'Repair layout':
            if not since_user:
                draft = {'widgets': [
                    {'id':'narrow_panel','description':'Action group','type':'Panel','props':{'title':'Actions','width':14,'padding':1},'children':['new_action']},
                    {'id':'new_action','description':'Set a result','type':'Button','props':{'label':'Set a value','hotkey':'Alt+V','width':'fill'},'on':{'press':{'action':'setState','params':{'statePath':'/result','value':'Updated'}}}}
                ]}
            elif json.loads(since_user[-1]['content']).get('error'):
                draft = {'updates':[{'id':'narrow_panel','key':'width','value':32}]}
                assert json.loads(since_user[-1]['content'])['pendingValidation'] is True
            else:
                draft = None
            message = {'tool_calls':[{'id':f'repair-{len(requests)}','type':'function','function':{'name':'edit_interface','arguments':json.dumps(draft)}}]} if draft else {'content':'Repaired parent width.'}
        elif request == 'Sixth turn edit':
            turn = sum(m['role'] == 'assistant' for m in since_user)
            name = 'inspect_interface' if turn < 5 else 'edit_interface'
            message = {'tool_calls':[{'id':f'last-turn-{len(requests)}','type':'function','function':{'name':name,'arguments':json.dumps({} if turn < 5 else draft)}}]}
        elif request == 'What can you do?':
            message = {'content': 'I can build terminal interfaces.'}
        elif since_user and request != 'invalid edit':
            message = {'content': 'Updated heading.'}
        else:
            message = {'tool_calls': [{'id': f'edit-{len(requests)}', 'type': 'function', 'function': {'name': 'edit_interface', 'arguments': json.dumps(draft)}}]}
        self.send_response(200)
        self.send_header('Content-Type', 'text/event-stream')
        self.end_headers()
        if request in ('Slow edit', 'Cancelled edit') and not since_user:
            self.wfile.write(('data: ' + json.dumps({'choices': [{'delta': {'content': 'Planning the edit…'}}]}) + '\n\n').encode())
            self.wfile.flush()
            time.sleep(1.5)
        try:
            for frame in [dict(choices=[dict(delta=message, finish_reason='tool_calls' if 'tool_calls' in message else 'stop')])]:
                self.wfile.write(('data: ' + json.dumps(frame) + '\n\n').encode())
            self.wfile.write(b'data: [DONE]\n\n')
        except (BrokenPipeError, ConnectionResetError):
            pass



def command(*args, env, ok=True):
    result = subprocess.run([str(BIN), *map(str, args), '--engine', 'llm', '--json'], capture_output=True, text=True, env=env, timeout=20)
    value = json.loads(result.stdout)
    assert value['ok'] == ok, (result.stdout, result.stderr)
    assert (result.returncode == 0) == ok
    return value


def terminal(command, env, act, cwd=None):
    master, slave = pty.openpty()
    fcntl.ioctl(slave, termios.TIOCSWINSZ, struct.pack('HHHH', 36, 100, 0, 0))
    process = subprocess.Popen(command, stdin=slave, stdout=slave, stderr=slave, env={**env, 'TERM': 'xterm-256color'}, cwd=cwd)
    os.close(slave)
    output = bytearray()

    def wait_for(predicate):
        deadline = time.monotonic() + 15
        while time.monotonic() < deadline:
            if select.select([master], [], [], .1)[0]:
                try:
                    output.extend(os.read(master, 65536))
                except OSError:
                    break
            if predicate(output):
                return
            assert process.poll() is None, output.decode(errors='replace')
        raise AssertionError(output.decode(errors='replace'))

    try:
        act(master, wait_for, process)
        os.write(master, b'\x03')
        deadline = time.monotonic() + 3
        while process.poll() is None and time.monotonic() < deadline:
            if select.select([master], [], [], .05)[0]:
                try: output.extend(os.read(master, 65536))
                except OSError: break
        assert process.wait(timeout=1) == 0
    finally:
        if process.poll() is None:
            process.kill()
            process.wait()
        os.close(master)


def main():
    server = ThreadingHTTPServer(('127.0.0.1', 0), Model)
    threading.Thread(target=server.serve_forever, daemon=True).start()
    env = {**os.environ, 'LLM_BASE_URL': f'http://127.0.0.1:{server.server_port}/v1', 'LLM_MODEL': 'fake', 'LLM_API_KEY': ''}
    try:
        with tempfile.TemporaryDirectory(prefix='tui-builder-') as temp:
            env['XDG_STATE_HOME'] = str(Path(temp) / 'state')
            project = Path(temp) / 'app'
            ui = json.loads((ROOT / 'examples/primitives.json').read_text())
            ui['elements']['sample']['props']['hotkey'] = 'Alt+R'
            (Path(temp) / 'hotkeys.json').write_text(json.dumps(ui))
            command('build', '--spec', Path(temp) / 'hotkeys.json', '--out', project, env=env)
            hook = project / 'src/actions.rs'
            hook.write_text('''use generated_tui_runtime::Spec;
pub fn handle(spec: &mut Spec, id: &str) -> Result<bool, String> {
    if id == "sample" { spec.state["result"] = serde_json::json!("CUSTOM HOOK WORKS"); return Ok(true); }
    Ok(false)
}
''')
            custom = hook.read_text()
            initial = command('inspect', project, env=env)['spec']
            edit = command('edit', project, 'Renamed from CLI', '--width', 120, '--height', 40, env=env)
            assert edit['revision'] == 2
            context = json.loads(requests[-1]['messages'][0]['content'].split('Current project (authoritative, overrides earlier tool snapshots): ')[1])
            assert context['originalGoal'] == 'Imported interface'
            assert context['viewport'] == [120, 40]
            assert edit['metrics']['layout']['viewport'] == [120, 40]
            layout = command('layout', project, '--width', 80, '--height', 24, env=env)['layout']
            assert layout['viewport'] == [80, 24] and 'heading' in layout['nodes']
            current = command('inspect', project, env=env)
            assert current['spec']['elements']['heading']['props']['text'] == 'Renamed from CLI'
            assert current['spec']['state'] == initial['state']
            assert hook.read_text() == custom
            source = (project / 'src/ui.rs').read_bytes()
            reply = command('edit', project, 'What can you do?', env=env)
            assert reply['revision'] == 2 and reply['changed'] is False
            assert reply['summary'] == 'I can build terminal interfaces.'
            assert (project / 'src/ui.rs').read_bytes() == source
            assert json.loads((project / '.builder/session.json').read_text())['chat'][-1]['reply'] == reply['summary']
            before = (project / '.builder/session.json').read_bytes()
            command('edit', project, 'invalid edit', env=env, ok=False)
            assert (project / '.builder/session.json').read_bytes() == before
            failure = json.loads((project / '.builder/last-failure.json').read_text())
            assert any('unsupportedProp' in m.get('content', '') for m in failure['messages'] if m['role'] == 'tool')

            def feedback(fd, wait, _):
                wait(lambda out: b'Session resumed' in out)
                os.write(fd, b'Renamed from resumed chat\r')
                wait(lambda _: json.loads((project / '.builder/session.json').read_text())['cursor'] == 2)
            terminal([str(BIN), 'chat', str(project), '--engine', 'llm'], env, feedback)
            current = command('inspect', project, env=env)
            assert current['revision'] == 3
            assert current['spec']['elements']['heading']['props']['text'] == 'Renamed from resumed chat'
            assert any(m.get('content') == 'Renamed from CLI' for m in requests[-1]['messages'])
            context = json.loads(requests[-1]['messages'][0]['content'].split('Current project (authoritative, overrides earlier tool snapshots): ')[1])
            assert context['viewport'] == [100, 35]  # Chat overlays the canvas; only the footer occupies a row.
            command('undo', project, env=env)
            assert command('inspect', project, env=env)['spec']['elements']['heading']['props']['text'] == 'Renamed from CLI'
            assert hook.read_text() == custom
            subprocess.run(['cargo', 'build', '--offline', '--manifest-path', str(project / 'Cargo.toml'), '--target-dir', str(ROOT / 'target')], check=True, timeout=120)
            runner = ROOT / 'target/debug/generated-terminal-app'
            subprocess.run([str(runner), '--check'], check=True)
            snapshot = subprocess.check_output([str(runner), '--snapshot'], text=True)
            assert 'Renamed from CLI' in snapshot and 'Sample + modifier' in snapshot

            def controls(fd, wait, process):
                wait(lambda out: b'Renamed from CLI' in out)
                os.write(fd, b'q')  # Typing q into Input must not quit the exported app.
                wait(lambda out: b'q' in out)
                assert process.poll() is None
                os.write(fd, b'\x7f\x1br')
                # Ratatui skips unchanged spaces with cursor moves in a diff frame.
                wait(lambda out: all(word in out for word in [b'CUSTOM', b'HOOK', b'WORKS']))
            terminal([str(runner)], env, controls)
            def queued(fd, wait, _):
                wait(lambda out: b'Session resumed' in out)
                os.write(fd, b'Slow edit\r')
                wait(lambda out: b'Planning' in out)
                os.write(fd, b'Queued feedback\r')
                wait(lambda out: b'queued' in out)
                wait(lambda _: json.loads((project / '.builder/session.json').read_text())['turns'][-1]['prompt'] == 'Queued feedback')
            terminal([str(BIN), 'chat', str(project), '--engine', 'llm'], env, queued)
            saved = (project / '.builder/session.json').read_bytes()
            def cancelled(fd, wait, _):
                wait(lambda out: b'Session resumed' in out)
                os.write(fd, b'Cancelled edit\r')
                wait(lambda out: b'Planning' in out)
                os.write(fd, b'\x18')
                wait(lambda out: b'Cancelled' in out)
            terminal([str(BIN), 'chat', str(project), '--engine', 'llm'], env, cancelled)
            assert (project / '.builder/session.json').read_bytes() == saved
            def preview_question(fd, wait, _):
                wait(lambda out: b'Session resumed' in out)
                os.write(fd, b'\x07')  # hide chat and focus first input
                os.write(fd, b'9\x07What can you do?\r')
                wait(lambda _: json.loads((project / '.builder/session.json').read_text())['chat'][-1]['prompt'] == 'What can you do?')
            terminal([str(BIN), 'chat', str(project), '--engine', 'llm'], env, preview_question)
            assert command('inspect', project, env=env)['spec']['state']['count'] == '29'
            revision = command('inspect', project, env=env)['revision']
            repaired = command('edit', project, 'Repair layout', env=env)
            assert repaired['revision'] == revision + 1
            repaired_spec = command('inspect', project, env=env)['spec']
            assert repaired_spec['elements']['narrow_panel']['props']['width'] == 32
            assert repaired_spec['elements']['narrow_panel']['children'] == ['new_action']
            assert repaired_spec['elements']['new_action']['props']['label'] == 'Set a value'
            cursor = json.loads((project / '.builder/session.json').read_text())['cursor']
            def last_turn(fd, wait, _):
                wait(lambda out: b'Session resumed' in out)
                os.write(fd, b'Sixth turn edit\r')
                wait(lambda _: json.loads((project / '.builder/session.json').read_text())['cursor'] == cursor + 1)
            terminal([str(BIN), 'chat', str(project), '--engine', 'llm'], env, last_turn)
            final = command('inspect', project, env=env)
            assert final['spec']['elements']['heading']['props']['text'] == 'Sixth turn edit'
            session = json.loads((project / '.builder/session.json').read_text())
            assert 'turn limit' in session['chat'][-1]['reply']
            assert len([m for m in session['chat'][-1]['messages'] if m['role'] == 'tool']) == 6
            print('PASS: streamed tool loop, conversation persistence, queued feedback, cancellation, rollback, undo, custom Rust and exported hotkeys')
    finally:
        server.shutdown()
        server.server_close()


if __name__ == '__main__':
    main()
