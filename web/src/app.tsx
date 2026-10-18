import { Component, type ReactNode } from 'react';
import { StyledEngineProvider } from '@mui/material/styles';
import ErrorIcon from '@mui/icons-material/Error';

import { handleHomePage } from './pages/home.js';

import './App.css';
import { Layout } from './layout.js';

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
    // Only used for initial asynchronous navigations
    controller: AbortController | null;

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

  pendingHandlerResult: Promise<HandleResult> | null = null;

  constructor(props: AppProps) {
    super(props);

    this.routes = [
      {
        pattern: new URLPattern({ baseURL: location.href, pathname: '/' }),
        handler: handleHomePage,
      },
      {
        pattern: new URLPattern({ baseURL: location.href, pathname: '/slow' }),
        handler: async (_params, _state, options) => {
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

    let initialResult = this.handleRoute(
      new URL(window.location.href),
      window.navigation.currentEntry?.getState(),
      new AbortController().signal,
    );

    let loading = null;
    let page = null;

    if (initialResult instanceof Promise) {
      let controller = new AbortController();
      loading = { controller, signal: controller.signal };
      this.pendingHandlerResult = initialResult;
    } else {
      document.title = initialResult.title;
      page = initialResult.contents;
    }

    this.state = {
      loading,
      page,
    };
  }

  handleRoute(url: URL, navigationEntryState: unknown, signal: AbortSignal): PossiblyPromise<HandleResult> {
    for (let route of this.routes) {
      let patternMatch = route.pattern.exec(url);

      if (patternMatch) {
        let params = patternMatch.pathname.groups as Record<string, string>;
        return route.handler(params, navigationEntryState, { signal });
      }
    }

    return {
      contents: (
        <Layout>
          <div className="ErrorState">
            <div className="icon">
              <ErrorIcon />
            </div>
            <h3>Page not found</h3>
          </div>
        </Layout>
      ),
      title: 'Page not found',
    };
  }

  override render() {
    return (
      // Put icon styles into the "mui" layer
      <StyledEngineProvider enableCssLayer>
        <div style={{ display: 'contents' }} inert={this.state.loading !== null}>
          {this.state.page}
        </div>
      </StyledEngineProvider>
    );
  }

  override componentDidMount() {
    this.controller = new AbortController();
    let appSignal = this.controller.signal;

    if (this.pendingHandlerResult) {
      let controller = this.state.loading!.controller!;

      this.pendingHandlerResult.then((result) => {
        if (controller.signal.aborted) {
          return;
        }

        document.title = result.title;
        this.setState({ loading: null, page: result.contents });
      }, (err) => {
        if (controller.signal.aborted && err instanceof DOMException && err.name === 'AbortError') {
          return;
        }

        console.error(err);
      });

      this.pendingHandlerResult = null;
    }

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

      let initialNavigationController = this.state.loading?.controller;

      if (initialNavigationController) {
        initialNavigationController.abort();
      }

      let url = new URL(event.destination.url);

      event.intercept({
        scroll: 'manual',
        handler: async () => {
          this.setState({
            loading: {
              controller: null,
              signal: event.signal,
            },
          });

          let result = await this.handleRoute(url, event.destination.getState(), event.signal);

          if (event.signal.aborted || appSignal.aborted) {
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
      }, { signal: appSignal });
    }, { signal: appSignal });


    document.addEventListener('scroll', () => {
      let scrollTop = document.documentElement.scrollTop;
      localStorage.setItem('scrollTop', scrollTop.toString());
    }, { signal: appSignal });

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
