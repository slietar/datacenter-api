import PowerIcon from '@mui/icons-material/Power';
import PowerOffIcon from '@mui/icons-material/PowerOff';
import StorageIcon from '@mui/icons-material/Storage';
import { OpsaLogo } from '@opsa-dev/ui-shared';
import { Application, NavBar } from '@opsa-dev/ui-tscl';
import '@opsa-dev/ui-tscl/styles/index.css';
import * as React from 'react';
import { Fragment } from 'react';
import useSWR from 'swr';
import './App.css';

const fetcher = (url: string) => fetch(url).then(res => res.json());


async function setPowerState(hostname: string, state: boolean) {
  await fetch(`${import.meta.env.VITE_API_URL || ""}/host/${hostname}/command`, {
    method: "PUT",
    body: JSON.stringify({ power: state }),
    headers: {
      'Content-Type': 'application/json'
    },
  });
}

interface Data {
  hosts: Record<string, {
    error: true;
  } | {
    error?: false;
    power_is_on: boolean;
    power_restore_policy: 'always-on' | 'always-off' | 'previous';
    sensors: Record<string, string>;
  }>;
}

export default function App() {
  let applicationRef = React.useRef<Application>(null);

  const { data }: { data: Data; } = useSWR(`${import.meta.env.VITE_API_URL || ""}/hosts`, fetcher, { refreshInterval: 5000 })

  return (
    <Application
      ref={applicationRef}
      brand={<OpsaLogo title="Datacenter" />}
      account={
        <a href="#" className="account">
          <img src="https://avatars.githubusercontent.com/u/11591121?v=4" />
          <div className="text">Simon</div>
        </a>
      }
      navigation={
        <NavBar
          leftEntries={[
            {
              id: 'servers',
              active: true,
              label: 'Servers',
              icon: StorageIcon,
              target: '/',
            },
          ]} />
      }>
      <div className="Body">
        <h1>Servers</h1>

        {data &&
          Object.entries(data.hosts)
            .toSorted(([k1, _v1], [k2, _v2]) => k1.localeCompare(k2))
            .map(([hostName, hostInfo]) => (
              <Fragment key={hostName}>
                <div className="Heading">
                  <h2>{hostName}</h2>
                  <div className="Actions">
                    {hostInfo.error
                      ? (
                        <button type="button" className="Button" disabled>
                          <PowerIcon />
                          <div className="text">Turn on</div>
                        </button>
                      )
                      : (
                        <button
                          type="button"
                          className="Button"
                          data-variant={hostInfo.power_is_on ? "danger" : "default"}
                          onClick={() => {
                            if (!hostInfo.error) {
                              applicationRef.current!.pushToast({
                                title: "Power state change on " + hostName,
                                description: `Turning ${hostName} ${hostInfo.power_is_on ? "off" : "on"}. This may take a few seconds.`,
                              });

                              setPowerState(hostName, !hostInfo.power_is_on);
                            }
                          }}>
                          {hostInfo.power_is_on ? <PowerOffIcon /> : <PowerIcon />}
                          <div className="text">{hostInfo.power_is_on ? "Turn off" : "Turn on"}</div>
                        </button>
                      )}
                  </div>
                </div>
                {hostInfo.error
                  ? (
                    <div className="EmptyState">
                      <p>This host is unavailable.</p>
                    </div>
                  )
                  : (
                    <div className="DataList">
                      <dl>
                        <dt>Power State</dt>
                        <dd>{hostInfo.power_is_on ? "On" : "Off"}</dd>
                        <dt>Power Restore Policy</dt>
                        <dd>{{
                          "always-on": "Always on",
                          "always-off": "Always off",
                          "previous": "Previous state"
                        }[hostInfo.power_restore_policy]
                        }</dd>
                      </dl>

                      <h3>Sensors</h3>
                      <dl>
                        {Object.entries(hostInfo.sensors)
                          .toSorted(([k1, _v1], [k2, _v2]) => k1.localeCompare(k2))
                          .map(([sensorName, sensorValue]) => (
                            <Fragment key={sensorName}>
                              <dt>{sensorName}</dt>
                              <dd>{sensorValue}</dd>
                            </Fragment>
                          ))}
                      </dl>
                    </div>
                  )}
              </Fragment>
            ))}
      </div>
    </Application>
  );
}
