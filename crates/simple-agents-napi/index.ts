import native from "./index.node";

export * from "./index.d";
export const { Client } = native as { Client: typeof import("./index.d").Client };
export default native;
