// A focusable, visually hidden IME receiver; all output goes to the mock desktop.
(() => {
  function create({ allowed, edit, key, status }) {
    const input = document.getElementById('remote-input-sink');
    let active = false;
    let composing = false;
    let baseline = '';

    function stop() {
      active = false;
      composing = false;
      baseline = '';
      input.value = '';
      if (document.activeElement === input) input.blur();
      status(false);
    }
    function canSend() {
      return active && allowed() && document.activeElement === input;
    }
    function flush(event) {
      if (!canSend() || composing || event?.isComposing) return;
      const previous = Array.from(baseline);
      const current = Array.from(input.value);
      let common = 0;
      while (common < previous.length && common < current.length && previous[common] === current[common]) common++;
      const remove = previous.length - common;
      const text = current.slice(common).join('');
      baseline = input.value;
      if (remove || text) edit(remove, text);
      input.setSelectionRange(input.value.length, input.value.length);
    }
    function command(name) {
      if (!canSend() || composing) return;
      if (name === 'Backspace') {
        edit(1, '');
        input.value = Array.from(baseline).slice(0, -1).join('');
        baseline = input.value;
      } else if (name === 'Enter' || name === 'Tab') {
        const text = name === 'Enter' ? '\n' : '\t';
        edit(0, text);
        input.value = baseline + text;
        baseline = input.value;
      } else key(name);
      input.setSelectionRange(input.value.length, input.value.length);
    }
    function activate() {
      if (!allowed()) { stop(); return; }
      if (active && composing) return;
      if (!active) { baseline = ''; input.value = ''; composing = false; }
      active = true;
      input.focus({ preventScroll: true });
      input.setSelectionRange(input.value.length, input.value.length);
      if (document.activeElement !== input) active = false;
      status(active);
    }
    function sync() {
      if (!allowed()) stop();
    }

    input.addEventListener('compositionstart', () => {
      if (!canSend()) return;
      composing = true;
    });
    input.addEventListener('compositionend', () => {
      if (!canSend()) { input.value = ''; return; }
      composing = false;
      flush();
    });
    input.addEventListener('input', flush);
    input.addEventListener('beforeinput', event => {
      if (!canSend()) { event.preventDefault(); return; }
      if (composing || event.isComposing) return;
      const commands = { deleteContentBackward: 'Backspace', deleteContentForward: 'Delete', insertLineBreak: 'Enter', insertParagraph: 'Enter' };
      const name = commands[event.inputType];
      if (name && event.cancelable) { event.preventDefault(); command(name); }
    });
    input.addEventListener('keydown', event => {
      // Candidate selection keys must remain with the local IME.
      if (composing || event.isComposing || event.keyCode === 229) { event.stopPropagation(); return; }
      if (!canSend()) return;
      if (event.ctrlKey || event.metaKey || event.altKey) {
        if (!['Control', 'Meta', 'Alt', 'Shift'].includes(event.key)) {
          event.preventDefault();
          key([event.ctrlKey ? 'Ctrl' : '', event.metaKey ? 'Win' : '', event.altKey ? 'Alt' : '', event.shiftKey ? 'Shift' : '', event.key].filter(Boolean).join(' + '));
        }
      } else if (['Tab', 'Escape', 'ArrowLeft', 'ArrowRight', 'ArrowUp', 'ArrowDown', 'Home', 'End', 'PageUp', 'PageDown'].includes(event.key)) {
        event.preventDefault(); command(event.key === 'Escape' ? 'Esc' : event.key);
      }
      event.stopPropagation();
    });
    input.addEventListener('blur', stop);
    document.addEventListener('pointerdown', event => {
      if (!active || event.target === input) return;
      if (event.target.closest('[data-action^="key-"]')) {
        event.preventDefault();
        return;
      }
      // Invalidate before blur can finish an in-flight composition.
      stop();
    }, true);
    window.addEventListener('blur', stop);
    document.addEventListener('visibilitychange', () => { if (document.hidden) stop(); });

    return { activate, stop, sync, command, isActive: () => active, isComposing: () => composing };
  }
  window.RemoteKeyboardPrototype = { create };
})();
