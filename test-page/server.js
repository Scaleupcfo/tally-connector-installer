// Serves index.html. Railway runs `npm start` which runs this.
// Local dev: `cd test-page && npm install && npm start` -> http://localhost:3000
const express = require('express');
const path = require('path');

const app = express();
const port = process.env.PORT || 3000;

app.use(express.static(__dirname));

app.listen(port, () => {
  console.log(`Lekha AI Tally Connector test page on port ${port}`);
});
