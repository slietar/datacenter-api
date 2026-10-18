import ErrorIcon from '@mui/icons-material/Error';
import { Copiable } from '@opsa-dev/ui-tscl';

import type { ClientResult } from '../bindings/ClientResult.js';
import type { HostsClientState } from '../bindings/HostsClientState.js';
import { formatRelativeInstant } from '../formatting.js';
import { Layout } from '../layout.js';
import { createStream, type Stream } from '../stream.js';


export async function handleHomePage() {
  let stream = await createStream<HostsClientState>('http://localhost:8080/hosts/events');

  return {
    contents: (
      <Layout activeNavigationEntryId="servers">
        <HomePage useHostsStream={stream} />
      </Layout>
    ),
    title: 'Home',
  };
}

function mapResult<T, S>(result: ClientResult<T>, mapper: (value: T) => S) {
  if (result.ok) {
    return mapper(result.value);
  }

  return (
    <div className="InlineError">
      <span className="icon">
        <ErrorIcon />
      </span>
      Error loading data
    </div>
  )
}

export function HomePage(props: {
  useHostsStream: Stream<HostsClientState>,
}) {
  let state = props.useHostsStream();

  return (
    <>
      <h1>Servers</h1>

      <table className="Table">
        <thead>
          <tr>
            <th>Name</th>
            <th>IP addresses</th>
            <th>Headscale status</th>
            <th>IPMI status</th>
            <th></th>
          </tr>
        </thead>
        <tbody>
          {Object.values(state?.hosts ?? {})
            .toSorted((a, b) => a.name.localeCompare(b.name))
            .map((host) => (
              <tr key={host.name}>
                <td>{host.name}</td>
                <td>
                  <Copiable contents={host.address}>{host.address}</Copiable>

                  {/* {host.headscale && host.headscale.ok && host.headscale.value
                    ? host.headscale.value.node.ipAddresses.map((address) => (
                      <Copiable key={address} contents={address}>{address}</Copiable>
                    ))
                    : (
                      <Copiable contents={host.address}>{host.address}</Copiable>
                    )
                  } */}
                </td>
                <td>
                  {host.headscale
                    ? mapResult(host.headscale, (headscaleState) => (
                      headscaleState
                        ? headscaleState.node.online
                          ? (
                            <>
                              <span className="Chip" data-variant="success" />
                              <span>Online</span>
                            </>
                          )
                          : (
                            <div className="ChipLayout">
                              <span className="Chip" data-variant="error" />
                              <div>
                                <div>Offline</div>
                                <div className="Subtext">Last seen {formatRelativeInstant(headscaleState.node.lastVisibilityTime * 1000)}</div>
                              </div>
                            </div>
                          )
                        : <></>
                    ))
                    : <>Not configured</>}
                </td>
                <td>
                  {host.ipmi && (
                    mapResult(host.ipmi, (ipmiState) => (
                      <>
                        <span className="Chip" data-variant={ipmiState.powerIsOn ? "success" : "error"} />
                        {ipmiState.powerIsOn
                          ? <>On</>
                          : <>Off</>
                        }
                      </>
                    ))
                  )}
                </td>
                <td style={{ width: '1%' }}>
                  <button type="button" className="Button">Manage</button>
                </td>
              </tr>
            ))}
        </tbody>
      </table>

      <pre>{JSON.stringify(state, null, 2)}</pre>
    </>
  );
}
