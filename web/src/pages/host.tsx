import SyncDisabledIcon from '@mui/icons-material/SyncDisabled';

import type { HostClientState } from '../bindings/HostClientState.js';
import { Layout } from '../layout.js';
import { createStream, type Stream } from '../stream.js';
import { formatRelativeInstant } from '../formatting.js';
import { useEffect, useRef } from 'react';
import { gravatarUrl } from '../utilities.js';


export async function handleHostPage(params: {
  hostName: string;
}) {
  let stream = await createStream<HostClientState>(`http://localhost:8080/host/${params.hostName}/events`);

  return {
    contents: (
      <Layout activeNavigationEntryId="servers">
        <HostPage useHostStream={stream} />
      </Layout>
    ),
    title: `Server ${stream.initialValue.name}`
  };
}

export function HostPage(props: {
  useHostStream: Stream<HostClientState>,
}) {
  let state = props.useHostStream();

  let headscale = state.headscale && state.headscale.ok && state.headscale.value
    ? state.headscale.value
    : null;

  let headscaleUserAvatarRef = useRef<HTMLImageElement | null>(null);

  useEffect(() => {
    if (headscale?.user.email) {
      gravatarUrl(headscale.user.email).then((url) => {
        if (headscaleUserAvatarRef.current) {
          headscaleUserAvatarRef.current.src = url;
        }
      });
    }
  }, [headscale?.user.email]);

  return (
    <>
      <div className="Breadcrumb">
        <div className="item">
          <a href="/">Servers</a>
        </div>
        <div className="item">
          <span>{state.name}</span>
        </div>
      </div>

      <h1>{state.name}</h1>

      <div className="Heading">
        <h2>Headscale status</h2>
        {headscale && (
          <div className="Actions">
            <a href={`https://headscale.lietar.net/admin/machines/${headscale.node.id}`} className="Button">Open in Headplane</a>
          </div>
        )}
      </div>

      {headscale
        ? (() => {
          let { node, user } = headscale;

          return (
            <div className="DataList">
              <h3>Information</h3>
              <dl>
                <dt>Name</dt>
                <dd>{node.name} ({node.id})</dd>
                <dt>Status</dt>
                <dd>
                  {node.online
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
                          <div className="Subtext">Last seen {formatRelativeInstant(node.lastVisibilityTime * 1000)}</div>
                        </div>
                      </div>
                    )}
                </dd>
                <dt>Addresses</dt>
                <dd>
                  {node.ipAddresses.map((address) => (
                    <div key={address}>{address}</div>
                  ))}
                </dd>
                <dt>Expiration</dt>
                <dd>
                  {node.expirationTime !== null
                    ? formatRelativeInstant(node.expirationTime * 1000)
                    : 'Never'}
                </dd>
                <dt>User</dt>
                <dd>
                  {user.email
                    ? (
                      <div className="User">
                        <img ref={headscaleUserAvatarRef} />
                        <div className="name">{user.displayName}</div>
                      </div>
                    )
                  : (
                    <>{user.displayName}</>
                  )}
                </dd>
              </dl>
            </div>
          );
        })()
        : (
          <div className="EmptyState">
            <div className="icon">
              <SyncDisabledIcon />
            </div>
            <p>Headscale is not configured on this server.</p>
          </div>
        )
      }

      <h2>Comin status</h2>

      <div className="EmptyState">
        <div className="icon">
          <SyncDisabledIcon />
        </div>
        <p>Comin is not configured on this server.</p>
      </div>

      <pre>{JSON.stringify(state, null, 2)}</pre>
    </>
  );
}
