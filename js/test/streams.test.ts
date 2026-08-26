import { describe, expect, it, vi } from "vitest";
import {
  asyncIterableToReadableStream,
  collect,
  readableStreamToAsyncIterable,
  transformStream,
} from "../src/streams.js";

const bytes = (...values: number[]) => new Uint8Array(values);

async function* source(...chunks: Uint8Array[]) {
  for (const chunk of chunks) yield chunk;
}

describe("streams", () => {
  it("collects chunks", async () => {
    expect(await collect(source(bytes(1, 2), bytes(), bytes(3, 4))))
      .toEqual(bytes(1, 2, 3, 4));
  });

  it("transforms every chunk", async () => {
    const result = await collect(transformStream(
      source(bytes(1, 2), bytes(3)),
      (chunk) => chunk.map((x) => x * 2),
    ));

    expect(result).toEqual(bytes(2, 4, 6));
  });

  it("converts an async iterable to a readable stream", async () => {
    const stream = asyncIterableToReadableStream(
      source(bytes(1), bytes(2, 3)),
    );

    expect(await collect(readableStreamToAsyncIterable(stream)))
      .toEqual(bytes(1, 2, 3));
  });

  it("propagates source errors", async () => {
    async function* failing() {
      yield bytes(1);
      throw new Error("boom");
    }

    await expect(collect(
      readableStreamToAsyncIterable(
        asyncIterableToReadableStream(failing()),
      ),
    )).rejects.toThrow("boom");
  });

  it("calls iterator.return when the stream is cancelled", async () => {
    const returnFn = vi.fn(async () => ({ done: true, value: undefined }));

    const iterator = {
      next: vi.fn(async () => ({ done: false, value: bytes(1) })),
      return: returnFn,
      [Symbol.asyncIterator]() {
        return this;
      },
    };

    const stream = asyncIterableToReadableStream(
      iterator as AsyncIterable<Uint8Array>,
    );

    await stream.cancel("test reason");
    expect(returnFn).toHaveBeenCalledWith("test reason");
  });
});
