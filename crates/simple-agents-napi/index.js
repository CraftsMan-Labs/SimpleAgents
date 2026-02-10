"use strict";

// Load the native addon built by napi-rs.
const native = require("./index.node");

module.exports = native;
module.exports.default = native;
