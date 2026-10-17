import { Component, type ReactNode } from 'react';

export type PossiblyPromise<T> = Promise<T> | T;
export type Handler = (
  params: Record<string, string>,
  state: unknown,
  options: { signal: AbortSignal; },
) => PossiblyPromise<HandleResult>;

export interface HandleResult {
  contents: ReactNode;
  title: string;
}

export function createRoute(pathname: string, handler: Handler) {
  return {
    pattern: new URLPattern({ baseURL: location.href, pathname }),
    handler,
  };
}

export interface AppProps { }

export interface AppState {
  loading: {
    // // Only used for initial asynchronous navigations
    // controller: AbortController | null;

    signal: AbortSignal;
  } | null;
  page: ReactNode;
}

export default class App extends Component<AppProps, AppState> {
  controller: AbortController | null = null;
  routes: {
    handler: Handler;
    pattern: URLPattern;
  }[];

  constructor(props: AppProps) {
    super(props);

    this.routes = [
      {
        pattern: new URLPattern({ baseURL: location.href, pathname: '/' }),
        handler: () => ({
          contents: (
            <div>
              <h1>Home</h1>
              <a href="/slow">Go to Slow Page</a>
            </div>
          ),
          title: 'Home',
        }),
      },
      {
        pattern: new URLPattern({ baseURL: location.href, pathname: '/slow' }),
        handler: async (_params, _state, options) => {
          options.signal.addEventListener('abort', () => {
            console.log('Navigation aborted');
          });

          await new Promise<void>((resolve, reject) => {
            let timeout = setTimeout(() => void resolve(), 1000);

            options.signal.addEventListener('abort', () => {
              clearTimeout(timeout);
              reject(new DOMException('Aborted', 'AbortError'));
            });
          });

          return {
            contents: (
              <div>
                <h1>Slow Page</h1>
                <a href="/">Go to Home</a>
              </div>
            ),
            title: 'Slow Page',
          };
        },
      }
    ];

    this.state = {
      loading: null,
      page: null,
    };
  }

  handleRoute(url: URL, navigationEntryState: unknown): PossiblyPromise<HandleResult> {
    for (let route of this.routes) {
      let patternMatch = route.pattern.exec(url);

      if (patternMatch) {
        let params = patternMatch.pathname.groups as Record<string, string>;
        return route.handler(params, navigationEntryState, { signal: new AbortController().signal });
      }
    }

    return {
      contents: (
        <div>
          <h1>Page Not Found</h1>
        </div>
      ),
      title: 'Page not found',
    };
  }

  override render() {
    return (
      <div style={{ display: 'contents' }} inert={this.state.loading !== null}>
        {this.state.page}
      </div>
    );
  }

  override componentDidMount() {
    this.controller = new AbortController();
    let signal = this.controller.signal;

    // Initial route
    Promise.resolve(this.handleRoute(new URL(window.location.href), window.navigation.currentEntry?.getState()))
      .then((result) => {
        if (!signal.aborted) {
          document.title = result.title;
          this.setState({ loading: null, page: result.contents });
        }
      });

    window.navigation.addEventListener('navigate', (event) => {
      if (!event.canIntercept) {
        return;
      }

      if (event.downloadRequest || event.formData) {
        return;
      }

      if (event.navigationType === 'reload') {
        return;
      }

      let url = new URL(event.destination.url);

      event.intercept({
        scroll: 'manual',
        handler: async () => {
          // Keep the previous page rendered (but inert) while the next one loads.
          this.setState({ loading: { signal: event.signal } });

          let result = await this.handleRoute(url, event.destination.getState());

          // Skip if this navigation was superseded or the component unmounted.
          if (event.signal.aborted || signal.aborted) {
            return;
          }

          await new Promise<void>((resolve) => {
            document.title = result.title;
            this.setState({ loading: null, page: result.contents }, () => void resolve());
          });
        },
      });

      event.signal.addEventListener('abort', () => {
        this.setState({ loading: null });
      }, { signal });
    }, { signal });


    document.addEventListener('scroll', () => {
      let scrollTop = document.documentElement.scrollTop;
      localStorage.setItem('scrollTop', scrollTop.toString());
    }, { signal });

    let savedScrollTop = parseInt(localStorage.getItem('scrollTop') ?? '0', 10);

    if (savedScrollTop) {
      document.documentElement.scrollTo({ top: savedScrollTop });
    }

    this.componentDidRender(null, null);
  }

  override componentDidUpdate(prevProps: Readonly<AppProps>, prevState: Readonly<AppState>): void {
    this.componentDidRender(prevProps, prevState);
  }

  override componentWillUnmount() {
    this.controller!.abort();
    this.controller = null;
  }

  componentDidRender(_prevProps: Readonly<AppProps> | null, _prevState: Readonly<AppState> | null) {
    // Custom logic to run after render
  }
}
