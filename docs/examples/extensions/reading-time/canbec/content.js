const article = document.querySelector("article");
if (article) {
  chrome.storage.sync.get({ wpm: 230 }, ({ wpm }) => {
    const words = article.innerText.split(/\s+/).length;
    const badge = document.createElement("div");
    badge.className = "reading-time";
    badge.textContent = `${Math.max(1, Math.round(words / wpm))} min read`;
    article.prepend(badge);
    // canbec: was chrome.runtime.sendMessage to background.js; counted here instead.
    chrome.storage.local.get({ articles: 0 }, ({ articles }) => chrome.storage.local.set({ articles: articles + 1 }));
  });
}
