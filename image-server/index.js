const http = require("http");
const sharp = require("sharp");

const PORT = 3000;
const RADIUS = 30;
const SIZE = 300;

const server = http.createServer(async (req, res) => {
  try {
    const url = new URL(req.url, `http://localhost:${PORT}`);

    if (url.pathname !== "/rounded") {
      res.writeHead(200, { "Content-Type": "text/plain" });
      res.end("Stremio Image Server\nUsage: /rounded?url=<image_url>&r=<radius>&s=<size>");
      return;
    }

    const imageUrl = url.searchParams.get("url");
    if (!imageUrl) {
      res.writeHead(400, { "Content-Type": "text/plain" });
      res.end("Missing url parameter");
      return;
    }

    const radius = parseInt(url.searchParams.get("r") || RADIUS);
    const size = parseInt(url.searchParams.get("s") || SIZE);

    // Görseli indir
    const response = await fetch(imageUrl, {
      headers: { "User-Agent": "StremioImageServer/1.0" },
    });

    if (!response.ok) {
      res.writeHead(502, { "Content-Type": "text/plain" });
      res.end("Failed to fetch image");
      return;
    }

    const buffer = Buffer.from(await response.arrayBuffer());

    // Köşeleri yuvarla
    const roundedCorners = Buffer.from(
      `<svg><rect x="0" y="0" width="${size}" height="${size}" rx="${radius}" ry="${radius}"/></svg>`
    );

    const result = await sharp(buffer)
      .resize(size, size, { fit: "cover" })
      .composite([
        {
          input: roundedCorners,
          blend: "dest-in",
        },
      ])
      .png()
      .toBuffer();

    res.writeHead(200, {
      "Content-Type": "image/png",
      "Cache-Control": "public, max-age=86400",
      "Access-Control-Allow-Origin": "*",
    });
    res.end(result);
  } catch (err) {
    console.error("Error:", err.message);
    res.writeHead(500, { "Content-Type": "text/plain" });
    res.end("Internal error");
  }
});

server.listen(PORT, () => {
  console.log(`Image server running on port ${PORT}`);
});
