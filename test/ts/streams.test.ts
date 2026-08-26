import { describe, expect, it } from "vitest";
import {
  asyncIterableToReadableStream,
  collect,
  readableStreamToAsyncIterable,
  transformStream,
} from "../src/index.js";

async function* source() {
  yield new Uint8Array([1, 2]);
  yield new Uint8Array([3]);
  yield new Uint8Array([4, 5]);
}

describe("streams", () => {
  it("collects chunks", async () => {
    await expect(collect(source()))
      .resolves.toEqual(new Uint8Array([1, 2, 3, 4, 5]));
  });

  it("transforms every chunk", async () => {
    const transformed = transformStream(source(), (chunk) =>
      chunk.map((value) => value + 1),
    );

    await expect(collect(transformed))
      .resolves.toEqual(new Uint8Array([2, 3, 4, 5, 6]));
  });

  it("converts an async iterable to a readable stream", async () => {
    const stream = asyncIterableToReadableStream(source());
    const result = await collect(readableStreamToAsyncIterable(stream));

    expect(result).toEqual(new Uint8Array([1, 2, 3, 4, 5]));
  });
});
