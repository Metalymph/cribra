import { resolve } from "node:path";

import HtmlWebpackPlugin from "html-webpack-plugin";

const appDir = process.env.CRIBRA_APP_DIR;
const outDir = process.env.CRIBRA_OUT_DIR;
if (!appDir || !outDir) {
  throw new Error("CRIBRA_APP_DIR and CRIBRA_OUT_DIR are required");
}

export default {
  mode: "production",
  context: appDir,
  entry: "./main.mjs",
  target: ["web", "es2022"],
  output: {
    path: outDir,
    filename: "assets/[name].[contenthash].js",
    chunkFilename: "assets/[name].[contenthash].js",
    publicPath: "/app/",
    clean: true,
    module: true,
    chunkFormat: "module",
  },
  experiments: {
    outputModule: true,
  },
  resolve: {
    conditionNames: ["browser", "import", "module", "default"],
  },
  module: {
    rules: [
      {
        test: /\.wasm$/,
        type: "asset/resource",
        generator: {
          filename: "assets/[name].[contenthash][ext]",
        },
      },
    ],
  },
  plugins: [
    new HtmlWebpackPlugin({
      template: resolve(appDir, "index.html"),
      filename: "index.html",
      inject: "body",
      scriptLoading: "module",
    }),
  ],
  optimization: {
    minimize: false,
  },
  devtool: false,
  stats: "errors-warnings",
};
