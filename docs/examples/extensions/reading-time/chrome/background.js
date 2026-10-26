// Counts articles read and shows the total on the toolbar badge.
chrome.runtime.onMessage.addListener((msg) => {
  if (msg.type !== "read") return;
  chrome.storage.local.get({ articles: 0 }, ({ articles }) => {
    chrome.storage.local.set({ articles: articles + 1 });
    chrome.action.setBadgeText({ text: String(articles + 1) });
  });
});
