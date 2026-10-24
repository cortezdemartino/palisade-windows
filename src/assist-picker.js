'use strict';
let selected = null;
const share = document.getElementById('share');
window.sourcePicker.onSources(sources => {
  for (const source of sources) {
    const label = document.createElement('label'), input = document.createElement('input');
    input.type = 'radio'; input.name = 'source'; input.value = source.id;
    input.addEventListener('change', () => { selected = source.id; share.disabled = false; });
    label.append(input);
    if (source.thumbnail.startsWith('data:image/png;base64,')) {
      const image = document.createElement('img'); image.src = source.thumbnail; image.alt = ''; label.append(image);
    }
    const text = document.createElement('span'); text.textContent = source.type + ': ' + source.name; label.append(text);
    document.getElementById('sources').append(label);
  }
});
share.addEventListener('click', () => { if (selected !== null) window.sourcePicker.choose(selected); });
document.getElementById('cancel').addEventListener('click', () => window.sourcePicker.cancel());
document.addEventListener('keydown', event => { if (event.key === 'Escape') window.sourcePicker.cancel(); });
