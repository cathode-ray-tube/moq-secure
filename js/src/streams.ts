export type ByteChunk = Uint8Array;
export type ByteSource = AsyncIterable<ByteChunk>;

export async function* readableStreamToAsyncIterable(
  stream: ReadableStream<ByteChunk>,
): AsyncGenerator<ByteChunk> {
  const reader = stream.getReader();

  try {
    while (true) {
      const item = await reader.read();
      if (item.done) return;
      yield item.value;
    }
  } finally {
    reader.releaseLock();
  }
}

export function asyncIterableToReadableStream(
  source: ByteSource,
): ReadableStream<ByteChunk> {
  const iterator = source[Symbol.asyncIterator]();

  return new ReadableStream<ByteChunk>({
    async pull(controller) {
      try {
        const item = await iterator.next();

        if (item.done) {
          controller.close();
        } else {
          controller.enqueue(item.value);
        }
      } catch (error) {
        controller.error(error);
      }
    },

    async cancel(reason) {
      await iterator.return?.(reason);
    },
  });
}

export async function* transformStream(
  source: ByteSource,
  transform: (chunk: ByteChunk) => Promise<ByteChunk> | ByteChunk,
): AsyncGenerator<ByteChunk> {
  for await (const chunk of source) {
    yield await transform(chunk);
  }
}

export async function collect(
  source: ByteSource,
): Promise<Uint8Array> {
  const chunks: ByteChunk[] = [];
  let length = 0;

  for await (const chunk of source) {
    chunks.push(chunk);
    length += chunk.length;
  }

  const result = new Uint8Array(length);
  let offset = 0;

  for (const chunk of chunks) {
    result.set(chunk, offset);
    offset += chunk.length;
  }

  return result;
}
