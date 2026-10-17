import { Component, type ReactNode } from 'react';

export interface AppProps { }

export interface AppState {
  loading: boolean;
  page: ReactNode;
}

export default class App extends Component<AppProps, AppState> {
  controller: AbortController | null = null;
  routes: {
    pattern: URLPattern;
    render: (params: Record<string, string>, state: unknown) => ReactNode | Promise<ReactNode>;
  }[];

  constructor(props: AppProps) {
    super(props);

    this.routes = [
      {
        pattern: new URLPattern({ baseURL: location.href, pathname: '/' }),
        render: (params: Record<string, string>, state: unknown) => {
          return (
            <div>
              <h1>Home</h1>
              <a href="/slow">Go to Slow Page</a>
            </div>
          );
        },
      },
      {
        pattern: new URLPattern({ baseURL: location.href, pathname: '/slow' }),
        render: async (params: Record<string, string>, state: unknown) => {
          await new Promise<void>(resolve => void setTimeout(() => void resolve(), 1000));

          return (
            <div>
              <h1>Slow Page</h1>
              <a href="/">Go to Home</a>
            </div>
          );
        },
      }
    ];

    this.state = {
      loading: true,
      page: null,
    };
  }

  renderRoute(url: URL, navigationEntryState: unknown): ReactNode | Promise<ReactNode> {
    for (let route of this.routes) {
      let patternMatch = route.pattern.exec(url);

      if (patternMatch) {
        let params = patternMatch.pathname.groups as Record<string, string>;
        return route.render(params, navigationEntryState);
      }
    }

    return (
      <div>
        <h1>Page Not Found</h1>
      </div>
    );
  }

  override render() {
    return (
      <div style={{ display: 'contents' }} inert={this.state.loading}>
        {this.state.page}
      </div>
    );
  }

  override componentDidMount() {
    this.controller = new AbortController();
    let signal = this.controller.signal;

    // Initial route
    Promise.resolve(this.renderRoute(new URL(window.location.href), window.navigation.currentEntry?.getState()))
      .then((page) => {
        if (!signal.aborted) {
          this.setState({ loading: false, page });
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

      interface NavigationState { }

      let url = new URL(event.destination.url);
      let isFreshEntry = ((event.navigationType === 'push') || (event.navigationType === 'replace')) && !event.hashChange;
      let currentEntryIndex = window.navigation.currentEntry?.index;
      let currentState = window.navigation.currentEntry?.getState() as NavigationState | undefined;
      let newState = event.destination.getState() as NavigationState | undefined;
      let isSameURL = url.href === window.location.href;

      let info = event.info as {
        trigger: 'navBar';
      } | undefined;


      if ((info?.trigger === 'navBar') && ('vibrate' in navigator)) {
        navigator.vibrate(16);
      }


      event.intercept({
        scroll: 'manual',
        handler: async () => {
          // Keep the previous page rendered (but inert) while the next one loads.
          this.setState({ loading: true });

          let page = await this.renderRoute(url, event.destination.getState());

          // Skip if this navigation was superseded or the component unmounted.
          if (event.signal.aborted || signal.aborted) {
            return;
          }

          await new Promise<void>((resolve) => {
            this.setState({ loading: false, page }, () => void resolve());
          });
        },
      });

      event.signal.addEventListener('abort', () => {
        this.setState({ loading: false });
      }, { signal });
    }, { signal });


    document.addEventListener('scroll', (event) => {
      let scrollTop = document.documentElement.scrollTop;
      localStorage.setItem('scrollTop', scrollTop.toString());
    }, { signal });

    let savedScrollTop = parseInt(localStorage.getItem('scrollTop') ?? '0', 10);

    if (savedScrollTop) {
      document.documentElement.scrollTo({ top: savedScrollTop });
    }

    this.componentDidRender(null, null);
  }

  override componentDidUpdate(prevProps: Readonly<AppProps>, prevState: Readonly<AppState>, snapshot?: any): void {
    this.componentDidRender(prevProps, prevState);
  }

  override componentWillUnmount() {
    this.controller!.abort();
    this.controller = null;
  }

  componentDidRender(prevProps: Readonly<AppProps> | null, prevState: Readonly<AppState> | null) {
    // Custom logic to run after render
  }
}
