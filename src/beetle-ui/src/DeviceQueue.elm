module DeviceQueue exposing (QueuePayloadKinds(..), postMessage)

import Environment as Env
import File
import Http
import Job
import Json.Encode as Encode


type QueuePayloadKinds
    = MessagePayload String
    | LinkPayload String
    | LightPayload Bool
    | SchedulePayload Bool
    | SendImage File.File
    | DeviceRenamePayload String
    | Refresh
    | Clear
    | WelcomeMessage
    | PublicAccessChange Bool


encodeStringPayloadWithKind : String -> String -> Encode.Value -> Encode.Value
encodeStringPayloadWithKind id kind content =
    Encode.object
        [ ( "device_id", Encode.string id )
        , ( "kind"
          , Encode.object
                [ ( "beetle:kind", Encode.string kind )
                , ( "beetle:content", content )
                ]
          )
        ]


postMessage : Env.Environment -> (Result Http.Error Job.JobHandle -> a) -> String -> QueuePayloadKinds -> Cmd a
postMessage env messageKind id payloadKind =
    let
        encoder =
            encodeStringPayloadWithKind id

        payload =
            case payloadKind of
                SendImage file ->
                    Http.fileBody file

                Refresh ->
                    Http.jsonBody
                        (Encode.object
                            [ ( "device_id", Encode.string id )
                            , ( "kind"
                              , Encode.object
                                    [ ( "beetle:kind", Encode.string "refresh" )
                                    ]
                              )
                            ]
                        )

                Clear ->
                    Http.jsonBody
                        (Encode.object
                            [ ( "device_id", Encode.string id )
                            , ( "kind"
                              , Encode.object
                                    [ ( "beetle:kind", Encode.string "clear_render" )
                                    ]
                              )
                            ]
                        )

                WelcomeMessage ->
                    Http.jsonBody
                        (Encode.object
                            [ ( "device_id", Encode.string id )
                            , ( "kind"
                              , Encode.object
                                    [ ( "beetle:kind", Encode.string "registration" )
                                    ]
                              )
                            ]
                        )

                DeviceRenamePayload newName ->
                    Http.jsonBody (encoder "rename" (Encode.string newName))

                LightPayload isOn ->
                    Http.jsonBody (encoder "lights" (Encode.bool isOn))

                SchedulePayload isOn ->
                    Http.jsonBody (encoder "schedule" (Encode.bool isOn))

                LinkPayload str ->
                    Http.jsonBody (encoder "link" (Encode.string str))

                PublicAccessChange value ->
                    Http.jsonBody
                        (Encode.object
                            [ ( "device_id", Encode.string id )
                            , ( "kind"
                              , Encode.object
                                    [ ( "beetle:kind"
                                      , Encode.string
                                            (if value then
                                                "make_public"

                                             else
                                                "make_private"
                                            )
                                      )
                                    ]
                              )
                            ]
                        )

                MessagePayload str ->
                    Http.jsonBody (encoder "message" (Encode.string str))
    in
    Http.post
        { url = Env.apiRoute env "device-queue" ++ "/" ++ id
        , body = payload
        , expect = Http.expectJson messageKind Job.handleDecoder
        }
