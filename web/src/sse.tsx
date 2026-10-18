export class ModernEventSource<T> {
  private eventSource: EventSource;
  private queue: T[] = [];
  private waiter: PromiseWithResolvers<void> | null = null;
  private closedResolve: () => void;

  readonly closed: Promise<void>;
  readonly ready: Promise<void>;

  constructor(url: URL | string) {
    this.eventSource = new EventSource(url.toString());

    let { promise: readyPromise, resolve: readyResolve, reject: readyReject } = Promise.withResolvers<void>();
    let { promise: closedPromise, resolve: closedResolve, reject: closedReject } = Promise.withResolvers<void>();

    this.ready = readyPromise;
    this.closed = closedPromise;
    this.closedResolve = closedResolve;

    this.eventSource.addEventListener('open', () => {
      readyResolve();

      this.eventSource.addEventListener('close', () => {
        this.closedResolve();
      });
    });

    this.eventSource.addEventListener('error', () => {
      readyReject(new Error('Failed to connect to EventSource'));
      closedReject(new Error('Failed to connect to EventSource'));
    }, { once: true });

    this.eventSource.addEventListener('message', (event) => {
      this.queue.push(JSON.parse(event.data) as T);

      if (this.waiter) {
        this.waiter.resolve();
        this.waiter = null;
      }
    });
  }

  close() {
    this.eventSource.close();
    this.closedResolve();
  }

  async event() {
    while (true) {
      if (this.queue.length > 0) {
        return this.queue.shift()!;
      } else {
        if (!this.waiter) {
          this.waiter = Promise.withResolvers();
        }

        await this.waiter.promise;
      }
    }
  }

  async * events() {
    while (true) {
      if (this.queue.length > 0) {
        yield this.queue.shift()!;
      } else {
        if (!this.waiter) {
          this.waiter = Promise.withResolvers();
        }

        await this.waiter.promise;
      }
    }
  }
}
