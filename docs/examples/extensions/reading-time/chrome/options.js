const input = document.getElementById("wpm");
chrome.storage.sync.get({ wpm: 230 }, ({ wpm }) => (input.value = wpm));
input.onchange = () => chrome.storage.sync.set({ wpm: Number(input.value) });
