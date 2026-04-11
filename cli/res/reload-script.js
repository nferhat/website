// reload-script.js -*- a quick reload script based on SSEs.
// This assumes the server (which is included in the CLI) exposes a /__reload__ route, which will
// send a reload event whenever needed

const es = new EventSource("/__reload__");

es.onmessage = (e) => {
  if (e.data === "reload") {
    window.location.reload();
    document.querySelectorAll('link[rel="stylesheet"]').forEach((link) => {
      const url = new URL(link.href);
      url.searchParams.set("v", Date.now()); // cache buster
      link.href = url.toString();
    });
  }
};

// optional: reconnect logging
es.onerror = () => {
  console.warn("Live reload disconnected, retrying...");
};

es.onready = () => console.info("Hot reloading is ready");
