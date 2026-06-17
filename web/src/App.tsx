import * as React from 'react';
import { Fragment } from 'react';
import StorageIcon from '@mui/icons-material/Storage';
import PowerOffIcon from '@mui/icons-material/PowerOff';
import PersonIcon from '@mui/icons-material/Person';
import PowerIcon from '@mui/icons-material/Power';
import { Tooltip, Table, TableHead, TableBody, TableRow, TableCell, IconButton, Collapse } from '@mui/material';
import PowerSettingsNewIcon from '@mui/icons-material/PowerSettingsNew'
import KeyboardArrowDownIcon from '@mui/icons-material/KeyboardArrowDown';
import KeyboardArrowUpIcon from '@mui/icons-material/KeyboardArrowUp';
import useSWR from 'swr'
import './App.css';
import '@opsa-design/shared/styles/css/index.css';
import '@opsa-design/management/styles/index.css';
import '@opsa-design/shared/scripts/components/logo.js';
import { Application, NavBar } from '@opsa-design/management';
import { OpsaLogo } from '@opsa-design/shared/scripts/components/logo.js';

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

function HostRow({ hostname, data }: {hostname: string, data: any}) {
  const [open, setOpen] = React.useState(false);

  return <>
    <TableRow>
      <TableCell>
        <IconButton aria-label="expand row" size="small" onClick={() => setOpen(!open)} disabled={!!data.error} >
          {open ? <KeyboardArrowUpIcon /> : <KeyboardArrowDownIcon />}
        </IconButton>
      </TableCell>
      <TableCell>{hostname}</TableCell>
      <TableCell>{data.error ? "Unavailable" : (data.power_is_on ? "On" : "Off")}</TableCell>
      <TableCell>
        <Tooltip title={data.power_is_on ? "Power Off" : "Power On"}>
          <IconButton
            disabled={!!data.error}
            color={data.power_is_on ? "error" : "success"}
            onClick={async () => { await setPowerState(hostname, !data.power_is_on); } }
          >
            <PowerSettingsNewIcon/>
          </IconButton>
        </Tooltip>
      </TableCell>
    </TableRow>
    <TableRow>
      <TableCell style={{ paddingBottom: 0, paddingTop: 0 }} colSpan={4}>
        {!data.error && <Collapse in={open} timeout="auto">
          <Table>
            <TableBody>
              { Object.entries(data.sensors)
                    .toSorted(([k1, _v1], [k2, _v2]) => k1.localeCompare(k2))
                    .map(([k,v]) => <SensorRow name={k} value={v as string} key={k} />) }
            </TableBody>
          </Table>
        </Collapse> }
      </TableCell>
    </TableRow>
  </>;
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

  const { data } = useSWR(`${import.meta.env.VITE_API_URL || ""}/hosts`, fetcher, { refreshInterval: 5000 })
  // const data: Data = {
  //     "hosts": {
  //         "gnb-003": {
  //             "power_is_on": true,
  //             "power_restore_policy": "always-off",
  //             "sensors": {
  //                 "FCB FAN3": "0.00 rpm",
  //                 "PSU 1 POUT": "0.00 A",
  //                 "FCB Ambient2": "0.00 °C",
  //                 "FCB Ambient1": "18.00 °C",
  //                 "FCB FAN4": "0.00 rpm",
  //                 "PSU 2 POUT": "170.00 A",
  //                 "FCB FAN2": "0.00 rpm",
  //                 "PS Current": "0.00 A",
  //                 "FCB FAN1": "0.00 rpm"
  //             }
  //         },
  //         "gnb-001": {
  //           "error": true
  //         },
  //         "gnb-002": {
  //             "power_is_on": false,
  //             "power_restore_policy": "always-off",
  //             "sensors": {
  //                 "FCB Ambient1": "18.00 °C",
  //                 "FCB Ambient2": "0.00 °C",
  //                 "FCB FAN3": "0.00 rpm",
  //                 "FCB FAN1": "0.00 rpm",
  //                 "FCB FAN2": "0.00 rpm",
  //                 "FCB FAN4": "0.00 rpm",
  //                 "PSU 2 POUT": "170.00 A",
  //                 "PS Current": "0.00 A",
  //                 "PSU 1 POUT": "0.00 A"
  //             }
  //         },
  //         "gnb-004": {
  //             "power_is_on": false,
  //             "power_restore_policy": "always-off",
  //             "sensors": {
  //                 "FCB Ambient2": "0.00 °C",
  //                 "FCB FAN3": "0.00 rpm",
  //                 "FCB Ambient1": "18.00 °C",
  //                 "PSU 1 POUT": "0.00 A",
  //                 "FCB FAN1": "0.00 rpm",
  //                 "PS Current": "0.00 A",
  //                 "FCB FAN2": "0.00 rpm",
  //                 "FCB FAN4": "0.00 rpm",
  //                 "PSU 2 POUT": "170.00 A"
  //             }
  //         }
  //     }
  // };

  return (
    <Application
      ref={applicationRef}
      brand={<OpsaLogo title="Datacenter" />}
      account={
        <a href="#" className="account">
          <img src="https://avatars.githubusercontent.com/u/11591121?v=4" />
          {/* <PersonIcon /> */}
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
