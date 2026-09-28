const portInput = document.getElementById("port");
const tokenInput = document.getElementById("token");
const status = document.getElementById("status");

chrome.storage.local.get(["port", "token"]).then((s) => {
  if (s.port) portInput.value = s.port;
  if (s.token) tokenInput.value = s.token;
});

document.getElementById("save").addEventListener("click", async () => {
  const port = parseInt(portInput.value, 10) || 8787;
  const token = tokenInput.value.trim();
  await chrome.storage.local.set({ port, token });
  status.textContent = "Enregistré ✓";
  setTimeout(() => (status.textContent = ""), 2000);
});
